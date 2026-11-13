// Implementation checks: every builder runs on the pinned mainnet binary (LiteSVM, v1 transactions),
// driven through the package's own fetch*/send/checkOutcome/resolveCreatedAccount via a LiteSVM-backed RPC.
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  AccountRole,
  type Address,
  address,
  decompileTransactionMessage,
  generateKeyPairSigner,
  getCompiledTransactionMessageDecoder,
  getTransactionDecoder,
  getTransactionMessageComputeUnitLimit,
  getTransactionMessageLoadedAccountsDataSizeLimit,
  getTransactionMessagePriorityFeeLamports,
  type Instruction,
  type KeyPairSigner,
  lamports,
  type Signature,
} from '@solana/kit';

import {
  type AccountSnapshot,
  buildCreateAccount,
  buildRemoveSpendingLimit,
  buildSetSigners,
  buildTransfer,
  checkOutcome,
  deriveProgramConfig,
  deriveSpendingLimit,
  deriveVault,
  fetchAccount,
  fetchBalances,
  fetchSpendingLimits,
  type Permission,
  resolveCreatedAccount,
  send,
  type SendOutcome,
  wrapTransaction,
} from '../../src/index.ts';
import { concat, pubkey, u64, u8, vec } from '../../src/encoding/bytes.ts';
import { i64 } from '../support/program.ts';
import { DISC } from '../../src/encoding/constants.ts';
import { deriveAssociatedTokenAccount } from '../../src/derive.ts';
import { createLiteSvmRpc, PROGRAM_ID } from './litesvm-rpc.ts';

const ALL: Permission[] = ['initiate', 'vote', 'execute'];
const SYSTEM = address('11111111111111111111111111111111');
const TOKEN = address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');

function assertConfirmed(o: SendOutcome): asserts o is Extract<SendOutcome, { status: 'confirmed' }> {
  assert.equal(o.status, 'confirmed', JSON.stringify(o, (_, v) => (typeof v === 'bigint' ? String(v) : v)));
}

/** Test-only: executeSettingsTransactionSync with one AddSpendingLimit action (out of src scope). */
function addSpendingLimitIx(p: {
  account: AccountSnapshot;
  signers: KeyPairSigner[];
  rentPayer: KeyPairSigner;
  seed: Address;
  spendingLimit: Address;
  mint: Address;
  amount: bigint;
  limitSigners: Address[];
  destinations: Address[];
}): Instruction {
  const action = concat(
    u8(4),
    pubkey(p.seed),
    u8(0),
    pubkey(p.mint),
    u64(p.amount),
    u8(1), // Day
    vec(p.limitSigners.map(pubkey)),
    vec(p.destinations.map(pubkey)),
    i64(1_800_000_000n),
  );
  return {
    programAddress: PROGRAM_ID,
    accounts: [
      { address: p.account.settings, role: AccountRole.WRITABLE },
      { address: p.rentPayer.address, role: AccountRole.WRITABLE_SIGNER, signer: p.rentPayer },
      { address: SYSTEM, role: AccountRole.READONLY },
      { address: PROGRAM_ID, role: AccountRole.READONLY },
      ...p.signers.map((s) => ({ address: s.address, role: AccountRole.READONLY_SIGNER, signer: s })),
      { address: p.spendingLimit, role: AccountRole.WRITABLE },
    ],
    data: concat(DISC.executeSettingsTransactionSync, u8(p.signers.length), vec([action]), u8(0)),
  };
}

async function setupAccount() {
  const env = createLiteSvmRpc();
  const { svm, rpc } = env;
  const creator = await generateKeyPairSigner();
  const privy = await generateKeyPairSigner();
  const turnkey = await generateKeyPairSigner();
  svm.airdrop(creator.address, lamports(20_000_000_000n));
  const pcAddress = await deriveProgramConfig();
  const pcAccount = svm.getAccount(pcAddress);
  assert.ok(pcAccount.exists);

  const signers = [
    { key: privy.address, permissions: ALL },
    { key: turnkey.address, permissions: ALL },
  ];
  const ours = await buildCreateAccount({ programConfig: pcAccount, creator, signers, threshold: 1 });
  assert.equal(ours.candidates.length, 25);

  // A concurrent creator takes the first candidate index before our transaction lands.
  const other = await generateKeyPairSigner();
  svm.airdrop(other.address, lamports(1_000_000_000n));
  const theirs = await buildCreateAccount({
    programConfig: pcAccount,
    creator: other,
    signers: [{ key: other.address, permissions: ALL }],
    threshold: 1,
    candidateCount: 1,
  });
  assertConfirmed(await send(rpc, { instructions: [theirs.instruction], feePayer: other, computeUnitLimit: theirs.computeUnitLimit }));

  const created = await send(rpc, { instructions: [ours.instruction], feePayer: creator, computeUnitLimit: ours.computeUnitLimit });
  assertConfirmed(created);
  const settings = await resolveCreatedAccount(rpc, created.signature, ours.candidates);
  assert.equal(settings, ours.candidates[1], 'resolved to the second candidate (first was taken)');

  const account = await fetchAccount(rpc, settings);
  return { ...env, creator, privy, turnkey, settings, account };
}

describe('LiteSVM on the mainnet binary (v1), through the package API', () => {
  it('buildCreateAccount (25 candidates) + resolveCreatedAccount + fetchAccount', async () => {
    const { account, privy, turnkey, settings } = await setupAccount();
    assert.equal(account.settings, settings);
    assert.equal(account.threshold, 1);
    assert.equal(account.timeLock, 0);
    assert.equal(account.settingsAuthority, null);
    assert.equal(account.transactionIndex, 0n);
    assert.deepEqual(
      [...account.signers].sort((a, b) => a.key.localeCompare(b.key)),
      [
        { key: privy.address, permissions: ALL },
        { key: turnkey.address, permissions: ALL },
      ].sort((a, b) => a.key.localeCompare(b.key)),
    );
  });

  it('fetchAccount rejects missing and foreign accounts', async () => {
    const { rpc, creator } = await setupAccount();
    await assert.rejects(fetchAccount(rpc, (await generateKeyPairSigner()).address), { code: 'ACCOUNT_NOT_FOUND' });
    await assert.rejects(fetchAccount(rpc, creator.address), { code: 'NOT_A_SMART_ACCOUNT' });
    await assert.rejects(fetchAccount(rpc, await deriveProgramConfig()), { code: 'NOT_A_SMART_ACCOUNT' });
  });

  it('buildTransfer SOL from vault 0, signed by one of two signers; fetchBalances', async () => {
    const { svm, rpc, txs, account, privy, creator } = await setupAccount();
    const vault = await deriveVault(account.settings, 0);
    svm.airdrop(vault, lamports(2_000_000_000n));
    const to = (await generateKeyPairSigner()).address;
    const ix = await buildTransfer({ account, vaultIndex: 0, signers: [privy], mint: 'SOL', to, amount: 123_456_789n });
    const outcome = await send(rpc, { instructions: [ix], feePayer: creator, priorityFeeLamports: 7_000n });
    assertConfirmed(outcome);
    assert.equal(svm.getBalance(to), 123_456_789n);
    const wire = txs.get(outcome.signature)?.wire ?? '';
    const sent = getTransactionDecoder().decode(Buffer.from(wire, 'base64'));
    assert.equal(sent.messageBytes[0], 0x81, 'v1 message');
    const msg = decompileTransactionMessage(getCompiledTransactionMessageDecoder().decode(sent.messageBytes));
    assert.ok(msg.version === 1);
    const cu = getTransactionMessageComputeUnitLimit(msg) ?? 0;
    const loaded = getTransactionMessageLoadedAccountsDataSizeLimit(msg) ?? 0;
    assert.ok(cu > 10_000 && cu < 400_000, `CU limit sized from simulation: ${cu}`);
    assert.ok(loaded > 1_600_000 && loaded % 32_768 === 0, `loaded-data limit sized from simulation: ${loaded}`);
    assert.equal(getTransactionMessagePriorityFeeLamports(msg), 7_000n);
    assert.equal((await fetchBalances(rpc, vault)).lamports, 2_000_000_000n - 123_456_789n);
    // fetchBalances takes the branded VaultAddress (from deriveVault); a plain Address or the settings address is a type error.
    // @ts-expect-error plain Address is not a VaultAddress
    await fetchBalances(rpc, to);
    // @ts-expect-error a SettingsAddress is not a VaultAddress
    await fetchBalances(rpc, account.settings);
  });

  it('buildTransfer SPL with createRecipientAta, paid by the vault; fetchBalances decodes it', async () => {
    const { svm, rpc, account, turnkey, creator } = await setupAccount();
    const vault = await deriveVault(account.settings, 0);
    svm.airdrop(vault, lamports(1_000_000_000n));
    const mint = (await generateKeyPairSigner()).address;
    const mintData = concat(new Uint8Array(4), new Uint8Array(32), u64(1_000_000n), u8(6), u8(1), new Uint8Array(36));
    svm.setAccount({ address: mint, data: mintData, executable: false, lamports: lamports(10_000_000n), programAddress: TOKEN, space: 82n });
    const getAta = (owner: Address) => deriveAssociatedTokenAccount(owner, mint, TOKEN);
    const source = await getAta(vault);
    const tokenData = concat(pubkey(mint), pubkey(vault), u64(1_000_000n), new Uint8Array(36), u8(1), new Uint8Array(12), u64(0n), new Uint8Array(36));
    svm.setAccount({ address: source, data: tokenData, executable: false, lamports: lamports(10_000_000n), programAddress: TOKEN, space: 165n });

    const to = (await generateKeyPairSigner()).address;
    const ix = await buildTransfer({
      account, vaultIndex: 0, signers: [turnkey.address], mint, to, amount: 250_000n, decimals: 6, createRecipientAta: true,
    });
    assertConfirmed(await send(rpc, { instructions: [ix], feePayer: creator, signers: [turnkey] }));
    const dest = svm.getAccount(await getAta(to));
    assert.ok(dest.exists);
    assert.equal(Buffer.from(dest.data).readBigUInt64LE(64), 250_000n);
    const balances = await fetchBalances(rpc, vault);
    assert.deepEqual(balances.tokens, [{ address: source, mint, tokenProgram: TOKEN, amount: 750_000n, decimals: 6 }]);
  });

  it('Grid email-account shape: buildSetSigners (2 full signers, threshold 1 → one new signer), then buildRemoveSpendingLimit with the new key', async () => {
    const { svm, rpc, account, privy, turnkey, creator } = await setupAccount();

    // A standing-order spending limit, created by the current signers (test-only encoder).
    const seed = (await generateKeyPairSigner()).address;
    const spendingLimit = await deriveSpendingLimit(account.settings, seed);
    const standingOrderKey = (await generateKeyPairSigner()).address;
    assertConfirmed(
      await send(rpc, {
        feePayer: creator,
        instructions: [
          addSpendingLimitIx({
            account, signers: [privy], rentPayer: creator, seed, spendingLimit, mint: SYSTEM, amount: 1_000_000n,
            limitSigners: [standingOrderKey], destinations: [creator.address],
          }),
        ],
      }),
    );
    const limits = await fetchSpendingLimits(rpc, account.settings);
    assert.equal(limits.length, 1);
    assert.equal(limits[0]?.address, spendingLimit);
    assert.equal(limits[0]?.period, 'day');
    assert.deepEqual(limits[0]?.signers, [standingOrderKey]);

    // Swap: privy + turnkey (threshold 1) → customer key only.
    const customer = await generateKeyPairSigner();
    const setIx = buildSetSigners({
      account, signers: [privy], newSigners: [{ key: customer.address, permissions: ALL }], newThreshold: 1, rentPayer: creator,
    });
    assertConfirmed(await send(rpc, { instructions: [setIx], feePayer: creator }));
    const after = await fetchAccount(rpc, account.settings);
    assert.deepEqual(after.signers, [{ key: customer.address, permissions: ALL }]);
    assert.equal(after.threshold, 1);

    // The old keys no longer satisfy the account.
    assert.throws(
      () => buildRemoveSpendingLimit({ account: after, signers: [privy], spendingLimit, rentPayer: creator }),
      { code: 'INSUFFICIENT_SIGNERS' },
    );

    // Cleanup with the new key; rent goes to rentPayer.
    const slLamports = svm.getBalance(spendingLimit) ?? 0n;
    assert.ok(slLamports > 0n);
    const payerBefore = svm.getBalance(creator.address) ?? 0n;
    const rmIx = buildRemoveSpendingLimit({ account: after, signers: [customer], spendingLimit, rentPayer: creator });
    assertConfirmed(await send(rpc, { instructions: [rmIx], feePayer: creator }));
    assert.equal(svm.getAccount(spendingLimit).exists, false);
    const payerAfter = svm.getBalance(creator.address) ?? 0n;
    assert.ok(payerAfter > payerBefore + slLamports - 100_000n, 'rent returned to rentPayer (minus fees)');
    assert.deepEqual(await fetchSpendingLimits(rpc, account.settings), []);
  });

  it('wrapTransaction strips compute budget and executes the rest', async () => {
    const { svm, rpc, account, privy, creator } = await setupAccount();
    const vault = await deriveVault(account.settings, 0);
    svm.airdrop(vault, lamports(1_000_000_000n));
    const to = (await generateKeyPairSigner()).address;
    const cb = address('ComputeBudget111111111111111111111111111111');
    const { instruction, stripped } = await wrapTransaction({
      account, vaultIndex: 0, signers: [privy],
      instructions: [
        { programAddress: cb, data: concat(u8(2), Uint8Array.of(0x40, 0x0d, 0x03, 0x00)) },
        { programAddress: cb, data: concat(u8(3), u64(1000n)) },
        {
          programAddress: SYSTEM,
          accounts: [{ address: vault, role: AccountRole.WRITABLE_SIGNER }, { address: to, role: AccountRole.WRITABLE }],
          data: concat(Uint8Array.of(2, 0, 0, 0), u64(5_000_000n)),
        },
      ],
    });
    assert.deepEqual(stripped, { unitLimit: 200_000, unitPrice: 1000n, priorityFeeLamports: 200n });
    assertConfirmed(await send(rpc, { instructions: [instruction], feePayer: creator }));
    assert.equal(svm.getBalance(to), 5_000_000n);
  });

  it('send: SIMULATION_FAILED when sizing by simulation; failed outcome when limits are given', async () => {
    const { svm, rpc, account, privy, creator } = await setupAccount();
    const vault = await deriveVault(account.settings, 0);
    svm.airdrop(vault, lamports(1_000_000n));
    const ix = await buildTransfer({ account, vaultIndex: 0, signers: [privy], mint: 'SOL', to: creator.address, amount: 10_000_000_000n });
    await assert.rejects(send(rpc, { instructions: [ix], feePayer: creator }), { code: 'SIMULATION_FAILED' });
    const outcome = await send(rpc, {
      instructions: [ix], feePayer: creator, computeUnitLimit: 200_000, loadedAccountsDataSizeLimit: 4 * 1024 * 1024,
    });
    assert.equal(outcome.status, 'failed');
    if (outcome.status === 'failed') assert.equal(outcome.error.instructionIndex, 0);
  });

  it('send returns unknown when the transaction never lands; checkOutcome says expired only past lastValidBlockHeight', async () => {
    const { svm, rpc, state, account, privy, creator } = await setupAccount();
    const vault = await deriveVault(account.settings, 0);
    svm.airdrop(vault, lamports(1_000_000_000n));
    const ix = await buildTransfer({ account, vaultIndex: 0, signers: [privy], mint: 'SOL', to: creator.address, amount: 1n });
    state.dropSends = true;
    state.blockHeight = state.lastValidBlockHeight + 1n; // so send stops polling at once
    const outcome = await send(rpc, { instructions: [ix], feePayer: creator });
    assert.equal(outcome.status, 'unknown');
    const sig: Signature = outcome.signature;
    state.blockHeight = state.lastValidBlockHeight;
    assert.equal((await checkOutcome(rpc, sig, state.lastValidBlockHeight)).status, 'unknown');
    state.blockHeight = state.lastValidBlockHeight + 1n;
    assert.equal((await checkOutcome(rpc, sig, state.lastValidBlockHeight)).status, 'expired');

    state.dropSends = false;
    state.blockHeight = 100n;
    const landed = await send(rpc, { instructions: [ix], feePayer: creator });
    assertConfirmed(landed);
    state.blockHeight = state.lastValidBlockHeight + 10n;
    assert.equal((await checkOutcome(rpc, landed.signature, state.lastValidBlockHeight)).status, 'confirmed');
  });

  it('send: TX_TOO_LARGE over 4096 bytes or 64 addresses, before simulating', async () => {
    const { rpc, creator } = await setupAccount();
    const memo = address('MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr');
    await assert.rejects(send(rpc, { feePayer: creator, instructions: [{ programAddress: memo, data: new Uint8Array(4100) }] }), {
      code: 'TX_TOO_LARGE',
    });
    const accounts = await Promise.all(
      Array.from({ length: 64 }, async () => ({ address: (await generateKeyPairSigner()).address, role: AccountRole.READONLY })),
    );
    await assert.rejects(send(rpc, { feePayer: creator, instructions: [{ programAddress: memo, accounts }] }), { code: 'TX_TOO_LARGE' });
  });
});

describe('resolveCreatedAccount when a griefer pre-funds candidate addresses (LiteSVM, mainnet binary)', () => {
  // The program's init accepts a pre-funded address (it tops up, allocates and assigns), so pre-funding cannot block
  // creation; it only hides the "zero before, positive after" signal.
  async function scenario(p: { prefund: bigint; rivalSameArgs?: boolean }) {
    const { svm, rpc, txs } = createLiteSvmRpc();
    const creator = await generateKeyPairSigner();
    const rival = await generateKeyPairSigner();
    const member = await generateKeyPairSigner();
    for (const k of [creator, rival]) svm.airdrop(k.address, lamports(20_000_000_000n));
    const pc = svm.getAccount(await deriveProgramConfig());
    assert.ok(pc.exists);
    const signers = [{ key: member.address, permissions: ALL }];
    const ours = await buildCreateAccount({ programConfig: pc, creator, signers, threshold: 1, candidateCount: 3 });
    // A rival takes the first index; the griefer pre-funds the second, which is where our transaction lands.
    const theirs = await buildCreateAccount({
      programConfig: pc, creator: rival, threshold: 1, candidateCount: 1,
      signers: p.rivalSameArgs ? signers : [{ key: rival.address, permissions: ALL }],
    });
    assertConfirmed(await send(rpc, { instructions: [theirs.instruction], feePayer: rival, computeUnitLimit: theirs.computeUnitLimit }));
    svm.airdrop(ours.candidates[1]!, lamports(p.prefund));
    const out = await send(rpc, { instructions: [ours.instruction], feePayer: creator, computeUnitLimit: ours.computeUnitLimit });
    assertConfirmed(out);
    const t = txs.get(out.signature)!;
    return { rpc, out, ours, t };
  }
  /** Safeguard removed: the strict "zero before, positive after" resolver. */
  const strictFunded = (t: { wire: string; pre: bigint[]; post: bigint[] }, candidates: readonly Address[]) => {
    const keys = getCompiledTransactionMessageDecoder().decode(getTransactionDecoder().decode(Buffer.from(t.wire, 'base64')).messageBytes).staticAccounts;
    return candidates.filter((c) => {
      const i = keys.indexOf(c);
      return i >= 0 && t.pre[i] === 0n && (t.post[i] ?? 0n) > 0n;
    });
  };

  it('pre-funded above rent: resolved by reading the candidates and matching this transaction\'s create arguments', async () => {
    const { rpc, out, ours, t } = await scenario({ prefund: 10_000_000n });
    assert.deepEqual(strictFunded(t, ours.candidates), [], 'safeguard removed: the strict resolver finds nothing');
    assert.equal(await resolveCreatedAccount(rpc, out.signature, ours.candidates), ours.candidates[1]);
    const account = await fetchAccount(rpc, ours.candidates[1]!);
    assert.equal(account.threshold, 1);
  });

  it('pre-funded below rent: resolved as the one candidate whose balance rose', async () => {
    const { rpc, out, ours, t } = await scenario({ prefund: 890_880n }); // rent-exempt for an empty account, below a Settings account's rent
    assert.deepEqual(strictFunded(t, ours.candidates), [], 'safeguard removed: the strict resolver finds nothing');
    assert.equal(await resolveCreatedAccount(rpc, out.signature, ours.candidates), ours.candidates[1]);
  });

  it('two candidates match the create arguments (a rival used identical arguments): ACCOUNT_NOT_FOUND, never a guess', async () => {
    const { rpc, out, ours } = await scenario({ prefund: 10_000_000n, rivalSameArgs: true });
    await assert.rejects(resolveCreatedAccount(rpc, out.signature, ours.candidates), { code: 'ACCOUNT_NOT_FOUND' });
  });
});
