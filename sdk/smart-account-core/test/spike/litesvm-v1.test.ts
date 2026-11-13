// LiteSVM spike: prove the pinned *mainnet* Smart Account Program binary runs under LiteSVM on
// Linux, driven by @solana/kit v1 transactions (SIMD-0385). Spike code: hand-rolled encoders, no
// SDK abstractions. Layouts follow the mainnet binary (which is the policies/"v2" build; see
// test/fixtures/mainnet/IDL-DIFF.md) — createSmartAccount and executeTransactionSync are unchanged
// from the IDL.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  AccountRole,
  type AccountMeta,
  type AccountSignerMeta,
  type Address,
  type Instruction,
  type KeyPairSigner,
  address,
  appendTransactionMessageInstructions,
  createTransactionMessage,
  generateKeyPairSigner,
  getAddressEncoder,
  getProgramDerivedAddress,
  lamports,
  pipe,
  setTransactionMessageComputeUnitLimit,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  setTransactionMessageLoadedAccountsDataSizeLimit,
  signTransactionMessageWithSigners,
} from '@solana/kit';
import { FailedTransactionMetadata, LiteSVM } from 'litesvm';

const PROGRAM_ID = address('SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG');
const SYSTEM_PROGRAM = address('11111111111111111111111111111111');
const COMPUTE_BUDGET_PROGRAM = address('ComputeBudget111111111111111111111111111111');
const FIXTURES = fileURLToPath(new URL('../fixtures/mainnet/', import.meta.url));
const PERMISSIONS_ALL = 0b111; // Initiate | Vote | Execute
const CANDIDATE_SETTINGS = 5;

// ---------- tiny encoders (spike only) ----------
const enc = new TextEncoder();
const addrEnc = getAddressEncoder();
const concat = (...parts: Uint8Array[]): Uint8Array => {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
};
const u8 = (n: number) => Uint8Array.of(n);
const u16 = (n: number) => {
  const b = new Uint8Array(2);
  new DataView(b.buffer).setUint16(0, n, true);
  return b;
};
const u32 = (n: number) => {
  const b = new Uint8Array(4);
  new DataView(b.buffer).setUint32(0, n, true);
  return b;
};
const u64 = (n: bigint) => {
  const b = new Uint8Array(8);
  new DataView(b.buffer).setBigUint64(0, n, true);
  return b;
};
const u128 = (n: bigint) => concat(u64(n & 0xffff_ffff_ffff_ffffn), u64(n >> 64n));
const readU128 = (d: Uint8Array, o: number) => {
  const v = new DataView(d.buffer, d.byteOffset);
  return v.getBigUint64(o, true) | (v.getBigUint64(o + 8, true) << 64n);
};
const pubkey = (a: Address) => new Uint8Array(addrEnc.encode(a));
const borshVec = (items: Uint8Array[]) => concat(u32(items.length), ...items);
const borshBytes = (b: Uint8Array) => concat(u32(b.length), b);
const NONE = u8(0);
const anchorDisc = (name: string) =>
  new Uint8Array(createHash('sha256').update(`global:${name}`).digest().subarray(0, 8));

// ---------- PDAs ----------
const pda = async (seeds: (string | Uint8Array)[]) =>
  (await getProgramDerivedAddress({ programAddress: PROGRAM_ID, seeds })).at(0) as Address;
const programConfigPda = () => pda(['smart_account', 'program_config']);
const settingsPda = (seed: bigint) => pda(['smart_account', 'settings', u128(seed)]);
const vaultPda = (settings: Address, index: number) =>
  pda(['smart_account', pubkey(settings), 'smart_account', u8(index)]);

// ---------- instructions ----------
function createSmartAccountIx(p: {
  programConfig: Address;
  treasury: Address;
  creator: KeyPairSigner;
  signers: Address[];
  threshold: number;
  candidateSettings: Address[];
}): Instruction {
  const signer = (a: Address) => concat(pubkey(a), u8(PERMISSIONS_ALL));
  const data = concat(
    anchorDisc('create_smart_account'),
    NONE, // settings_authority: None => autonomous
    u16(p.threshold),
    borshVec(p.signers.map(signer)),
    u32(0), // time_lock
    NONE, // rent_collector
    NONE, // memo
  );
  const accounts: (AccountMeta | AccountSignerMeta)[] = [
    { address: p.programConfig, role: AccountRole.WRITABLE },
    { address: p.treasury, role: AccountRole.WRITABLE },
    { address: p.creator.address, role: AccountRole.WRITABLE_SIGNER, signer: p.creator },
    { address: SYSTEM_PROGRAM, role: AccountRole.READONLY },
    { address: PROGRAM_ID, role: AccountRole.READONLY },
    ...p.candidateSettings.map((a) => ({ address: a, role: AccountRole.WRITABLE })),
  ];
  return { programAddress: PROGRAM_ID, accounts, data };
}

/** SmallVec<u8, CompiledInstruction>: u8 count, then {u8 programIdIndex, SmallVec<u8,u8> accounts, SmallVec<u16,u8> data}. */
function compileSyncInstructions(ixs: { programIdIndex: number; accountIndexes: number[]; data: Uint8Array }[]) {
  return concat(
    u8(ixs.length),
    ...ixs.map((ix) =>
      concat(u8(ix.programIdIndex), u8(ix.accountIndexes.length), Uint8Array.from(ix.accountIndexes), u16(ix.data.length), ix.data),
    ),
  );
}

function executeTransactionSyncIx(p: {
  settings: Address;
  signers: KeyPairSigner[];
  remaining: { address: Address; role: AccountRole }[];
  compiled: Uint8Array;
  accountIndex: number;
}): Instruction {
  const data = concat(
    anchorDisc('execute_transaction_sync'),
    u8(p.accountIndex),
    u8(p.signers.length),
    borshBytes(p.compiled),
  );
  const accounts: (AccountMeta | AccountSignerMeta)[] = [
    { address: p.settings, role: AccountRole.READONLY },
    { address: PROGRAM_ID, role: AccountRole.READONLY },
    ...p.signers.map((s): AccountSignerMeta => ({ address: s.address, role: AccountRole.READONLY_SIGNER, signer: s })),
    ...p.remaining,
  ];
  return { programAddress: PROGRAM_ID, accounts, data };
}

const systemTransferData = (amount: bigint) => concat(u32(2), u64(amount));

// ---------- harness ----------
async function setup() {
  const svm = new LiteSVM();
  svm.addProgram(PROGRAM_ID, readFileSync(`${FIXTURES}smart_account_program.so`));

  const pc = JSON.parse(readFileSync(`${FIXTURES}program_config.json`, 'utf8')) as {
    address: string;
    lamports: string;
    dataBase64: string;
    decoded: { smartAccountIndex: string; treasury: string };
  };
  const programConfig = await programConfigPda();
  assert.equal(programConfig, pc.address, 'ProgramConfig PDA seeds');
  const pcData = new Uint8Array(Buffer.from(pc.dataBase64, 'base64'));
  svm.setAccount({
    address: programConfig,
    data: pcData,
    executable: false,
    lamports: lamports(BigInt(pc.lamports)),
    programAddress: PROGRAM_ID,
    space: BigInt(pcData.length),
  });
  const treasury = address(pc.decoded.treasury);
  svm.airdrop(treasury, lamports(1_000_000_000n));
  return { svm, programConfig, treasury, smartAccountIndex: BigInt(pc.decoded.smartAccountIndex) };
}

type Budget = { computeUnitLimit?: number; loadedAccountsDataSizeLimit?: number };
const DEFAULT_BUDGET: Budget = { computeUnitLimit: 400_000, loadedAccountsDataSizeLimit: 4 * 1024 * 1024 };
const NO_BUDGET: Budget = {};

async function buildTx(svm: LiteSVM, version: 0 | 1, payer: KeyPairSigner, ixs: Instruction[], budget = DEFAULT_BUDGET) {
  let msg = pipe(
    createTransactionMessage({ version }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: svm.latestBlockhash(), lastValidBlockHeight: 0n }, m),
    (m) => appendTransactionMessageInstructions(ixs, m),
  );
  // v1: CU limit / loaded-accounts size live in the message config (no ComputeBudget ixs).
  // v0/legacy: kit's setters emit ComputeBudget instructions instead.
  if (budget.computeUnitLimit !== undefined) msg = setTransactionMessageComputeUnitLimit(budget.computeUnitLimit, msg);
  if (budget.loadedAccountsDataSizeLimit !== undefined)
    msg = setTransactionMessageLoadedAccountsDataSizeLimit(budget.loadedAccountsDataSizeLimit, msg);
  const tx = await signTransactionMessageWithSigners(msg);
  // Wire-format check: legacy has no prefix; versioned messages start with 0x80 | version.
  assert.equal(tx.messageBytes[0], 0x80 | version, `message is v${version}`);
  // v1 carries the budget in the message config, so no ComputeBudget instruction may appear.
  const hasComputeBudgetIx = msg.instructions.some((ix) => ix.programAddress === COMPUTE_BUDGET_PROGRAM);
  assert.equal(hasComputeBudgetIx, version === 0 && Object.keys(budget).length > 0, 'ComputeBudget ixs only for v0');
  return tx;
}

async function send(svm: LiteSVM, version: 0 | 1, payer: KeyPairSigner, ixs: Instruction[]) {
  const res = svm.sendTransaction(await buildTx(svm, version, payer, ixs));
  svm.expireBlockhash();
  return res;
}

function assertOk(res: unknown, label: string) {
  if (res instanceof FailedTransactionMetadata) {
    assert.fail(`${label} failed: ${res.toString()}\n${res.meta().logs().join('\n')}`);
  }
  return res;
}

for (const version of [1, 0] as const) {
  describe(`mainnet smart_account_program in LiteSVM (tx version ${version})`, () => {
    it('creates an autonomous 1-of-2 smart account and transfers SOL from vault 0 via executeTransactionSync', async () => {
      const { svm, programConfig, treasury, smartAccountIndex } = await setup();
      const creator = await generateKeyPairSigner();
      const signerA = await generateKeyPairSigner(); // e.g. Grid's session/MPC signer
      const signerB = await generateKeyPairSigner(); // e.g. Grid's backup/recovery signer
      svm.airdrop(creator.address, lamports(10_000_000_000n));

      const candidates = await Promise.all(
        Array.from({ length: CANDIDATE_SETTINGS }, (_, i) => settingsPda(smartAccountIndex + 1n + BigInt(i))),
      );
      const settings = candidates[0] as Address;

      // (1) createSmartAccount
      assertOk(
        await send(svm, version, creator, [
          createSmartAccountIx({
            programConfig,
            treasury,
            creator,
            signers: [signerA.address, signerB.address].sort() as Address[],
            threshold: 1,
            candidateSettings: candidates,
          }),
        ]),
        'createSmartAccount',
      );
      const settingsAcc = svm.getAccount(settings);
      assert.ok(settingsAcc.exists, 'settings account created');
      assert.equal(settingsAcc.programAddress, PROGRAM_ID);
      assert.equal(Buffer.from(settingsAcc.data.subarray(0, 8)).toString('hex'), 'dfb3a3beb1e043ad', 'Settings discriminator');
      // 193 = Settings::size(2) per fork/IDL (reserved1: u8); mainnet allocates +8 for policy_seed: Option<u64>.
      assert.equal(settingsAcc.space, 201n, 'Settings allocation (policy_seed: Option<u64> layout)');
      assert.equal(readU128(settingsAcc.data, 8), smartAccountIndex + 1n, 'settings.seed');
      const pcAfter = svm.getAccount(programConfig);
      assert.ok(pcAfter.exists);
      assert.equal(readU128(pcAfter.data, 8), smartAccountIndex + 1n, 'program_config.smart_account_index incremented');

      // (2) fund vault 0 and execute a SOL transfer from it
      const vault = await vaultPda(settings, 0);
      svm.airdrop(vault, lamports(2_000_000_000n));
      const destination = (await generateKeyPairSigner()).address;
      const amount = 123_456_789n;

      // remaining_accounts = [signers..., systemProgram, vault (w), destination (w)]
      const signers = [signerA, signerB];
      const remaining = [
        { address: SYSTEM_PROGRAM, role: AccountRole.READONLY },
        { address: vault, role: AccountRole.WRITABLE },
        { address: destination, role: AccountRole.WRITABLE },
      ];
      const base = signers.length;
      const compiled = compileSyncInstructions([
        { programIdIndex: base, accountIndexes: [base + 1, base + 2], data: systemTransferData(amount) },
      ]);
      const vaultBefore = svm.getBalance(vault) ?? 0n;
      assertOk(
        await send(svm, version, creator, [
          executeTransactionSyncIx({ settings, signers, remaining, compiled, accountIndex: 0 }),
        ]),
        'executeTransactionSync',
      );
      assert.equal(svm.getBalance(destination), amount, 'destination received');
      assert.equal(svm.getBalance(vault), vaultBefore - amount, 'vault debited');
    });
  });
}

describe('mainnet binary instruction surface (Anchor dispatcher probe, v1 tx)', () => {
  // Each instruction is sent with only its 8-byte discriminator: a known handler logs
  // "Instruction: X" and then fails deserialization/account checks; an unknown one fails with
  // InstructionFallbackNotFound (101). This is how IDL-DIFF.md establishes that the on-chain
  // binary is newer than the on-chain IDL.
  it('recognises all 37 IDL instructions plus execute_transaction_sync_v2, and nothing bogus', async () => {
    const { svm } = await setup();
    const payer = await generateKeyPairSigner();
    svm.airdrop(payer.address, lamports(10_000_000_000n));
    const idl = JSON.parse(readFileSync(`${FIXTURES}idl.json`, 'utf8')) as { instructions: { name: string }[] };
    const snake = (n: string) => n.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
    const probe = async (name: string) => {
      const ix: Instruction = { programAddress: PROGRAM_ID, accounts: [], data: anchorDisc(name) };
      const res = svm.simulateTransaction(await buildTx(svm, 1, payer, [ix]));
      return res.meta().logs().join('\n');
    };
    for (const name of [...idl.instructions.map((i) => snake(i.name)), 'execute_transaction_sync_v2']) {
      const logs = await probe(name);
      assert.match(logs, /Program log: Instruction: /, `${name} should be dispatched`);
      assert.doesNotMatch(logs, /InstructionFallbackNotFound/, name);
    }
    assert.match(await probe('definitely_not_an_instruction'), /InstructionFallbackNotFound/);
  });
});

describe('LiteSVM v1 defaults (observed behaviour, litesvm 1.6.2)', () => {
  // With no message config, a v1 tx gets a 0 loaded-accounts-data-size budget (and 0 CUs), so
  // loading a 1.6 MB program fails before execution. SDK builders must always set both.
  it('v1 without loadedAccountsDataSizeLimit fails with MaxLoadedAccountsDataSizeExceeded', async () => {
    const { svm } = await setup();
    const payer = await generateKeyPairSigner();
    svm.airdrop(payer.address, lamports(10_000_000_000n));
    const ix: Instruction = { programAddress: PROGRAM_ID, accounts: [], data: anchorDisc('log_event') };
    const res = svm.simulateTransaction(await buildTx(svm, 1, payer, [ix], NO_BUDGET));
    assert.ok(res instanceof FailedTransactionMetadata);
    assert.equal(res.err(), 29 /* TransactionErrorFieldless.MaxLoadedAccountsDataSizeExceeded */);
  });

  it('v1 with a data-size limit but no CU limit runs with a 0 CU budget', async () => {
    const { svm } = await setup();
    const payer = await generateKeyPairSigner();
    svm.airdrop(payer.address, lamports(10_000_000_000n));
    const ix: Instruction = { programAddress: PROGRAM_ID, accounts: [], data: anchorDisc('log_event') };
    const res = svm.simulateTransaction(
      await buildTx(svm, 1, payer, [ix], { loadedAccountsDataSizeLimit: 4 * 1024 * 1024 }),
    );
    assert.ok(res instanceof FailedTransactionMetadata);
    assert.match(res.meta().logs().join('\n'), /consumed 0 of 0 compute units/);
  });
});
