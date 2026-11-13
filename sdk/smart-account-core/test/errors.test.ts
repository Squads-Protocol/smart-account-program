// Every §4 error code is reachable from a documented precondition (spec §4, §5; plan §5.1).
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';

import {
  AccountRole,
  type Address,
  type Instruction,
  appendTransactionMessageInstructions,
  blockhash,
  compileTransaction,
  createTransactionMessage,
  decompileTransactionMessage,
  getCompiledTransactionMessageDecoder,
  partiallySignTransaction,
  type TransactionModifyingSigner,
  generateKeyPairSigner,
  getBase64EncodedWireTransaction,
  lamports,
  pipe,
  setTransactionMessageComputeUnitLimit,
  setTransactionMessageFeePayer,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  setTransactionMessageLoadedAccountsDataSizeLimit,
  signTransactionMessageWithSigners,
  getSignatureFromTransaction,
} from '@solana/kit';

import { rng } from './support/prng.ts';
import { DISC, PROGRAM_ID, SYSTEM_PROGRAM, TOKEN_PROGRAM, concat, pubkey, settingsPda, u32, u64 } from './support/program.ts';
import { accountsRpc, makeRpc } from './support/rpc.ts';
import { type AccountSnapshot, ERROR_CODES, type ErrorCode, sdk } from './support/sdk.ts';
import { createSvmRpc, setupSvm } from './support/svm.ts';
import { entry, noop, rejectsWith, unhex } from './support/util.ts';

const r = rng(20261112);
const [S, A, B, C, X] = Array.from({ length: 5 }, () => r.address()) as [Address, Address, Address, Address, Address];
const account = (p: Partial<AccountSnapshot> = {}): AccountSnapshot => ({
  settings: S, slot: 1n, threshold: 1, timeLock: 0, settingsAuthority: null, signers: [entry(A, 7), entry(B, 7)], transactionIndex: 0n, ...p,
});
const vaultOf = async () => {
  const { vaultPda } = await import('./support/program.ts');
  return vaultPda(S, 0);
};
const transferFromVault = (vault: Address, to: Address = C): Instruction => ({
  programAddress: SYSTEM_PROGRAM,
  accounts: [{ address: vault, role: AccountRole.WRITABLE_SIGNER }, { address: to, role: AccountRole.WRITABLE }],
  data: concat(u32(2), u64(1n)),
});
const SETTINGS_DATA = unhex(
  (JSON.parse(readFileSync(new URL('./vectors/grid/borsh_settings_policy_seed_none.json', import.meta.url), 'utf8')) as { expected: { accountDataHex: string } }).expected.accountDataHex,
);
const createBase = () => ({
  programConfig: { address: X, smartAccountIndex: 41n, treasury: C, authority: C, smartAccountCreationFee: 0n },
  creator: noop(A), signers: [entry(A, 7)], threshold: 1, timeLock: 0, settingsAuthority: null, rentCollector: null, memo: null,
});

interface Case {
  code: ErrorCode;
  pre: string;
  run: () => unknown;
}
const CASES: Case[] = [
  {
    code: 'ACCOUNT_NOT_FOUND',
    pre: 'fetchAccount: the settings account does not exist',
    run: () => sdk.fetchAccount(accountsRpc(new Map()).rpc, S),
  },
  {
    code: 'ACCOUNT_NOT_FOUND',
    pre: 'resolveCreatedAccount: the transaction funded none of the candidates',
    run: async () => {
      const { svm } = await setupSvm();
      const net = createSvmRpc(svm);
      const payer = await generateKeyPairSigner();
      svm.airdrop(payer.address, lamports(1_000_000_000n));
      const tx = await signTransactionMessageWithSigners(
        pipe(
          createTransactionMessage({ version: 1 }),
          (m) => setTransactionMessageFeePayerSigner(payer, m),
          (m) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: svm.latestBlockhash(), lastValidBlockHeight: 2_000n }, m),
          (m) => appendTransactionMessageInstructions([transferFromPayer(payer.address)], m),
          (m) => setTransactionMessageComputeUnitLimit(10_000, m),
          (m) => setTransactionMessageLoadedAccountsDataSizeLimit(1_000_000, m),
        ),
      );
      const rpc = net.rpc as unknown as { sendTransaction(w: string, c: object): { send(): Promise<unknown> } };
      await rpc.sendTransaction(getBase64EncodedWireTransaction(tx), { encoding: 'base64' }).send();
      const candidates = await Promise.all([1n, 2n, 3n].map((i) => settingsPda(745_853n + i)));
      return sdk.resolveCreatedAccount(net.rpc, getSignatureFromTransaction(tx), candidates);
    },
  },
  {
    code: 'NOT_A_SMART_ACCOUNT',
    pre: 'fetchAccount: Settings bytes but not owned by the program',
    run: () => sdk.fetchAccount(accountsRpc(new Map([[S, { data: SETTINGS_DATA, owner: SYSTEM_PROGRAM }]])).rpc, S),
  },
  {
    code: 'NOT_A_SMART_ACCOUNT',
    pre: 'fetchAccount: owned by the program but without the Settings discriminator',
    run: () => sdk.fetchAccount(accountsRpc(new Map([[S, { data: concat(DISC.spendingLimit, SETTINGS_DATA.subarray(8)), owner: PROGRAM_ID }]])).rpc, S),
  },
  {
    code: 'INSUFFICIENT_SIGNERS',
    pre: 'buildVaultTransaction: fewer voters than the threshold',
    run: async () => sdk.buildVaultTransaction({ account: account({ threshold: 2 }), vaultIndex: 0, signers: [noop(A)], instructions: [transferFromVault(await vaultOf())] }),
  },
  {
    code: 'INSUFFICIENT_SIGNERS',
    pre: 'buildVaultTransaction: signers lack the execute permission',
    run: async () =>
      sdk.buildVaultTransaction({ account: account({ signers: [entry(A, 3), entry(B, 7)] }), vaultIndex: 0, signers: [noop(A)], instructions: [transferFromVault(await vaultOf())] }),
  },
  {
    code: 'INSUFFICIENT_SIGNERS',
    pre: 'buildRemoveSpendingLimit: signers below the threshold',
    run: () => sdk.buildRemoveSpendingLimit({ account: account({ threshold: 2 }), signers: [noop(A)], spendingLimit: X, rentPayer: noop(C) }),
  },
  {
    code: 'SYNC_NOT_ALLOWED',
    pre: 'buildVaultTransaction: time lock > 0',
    run: async () => sdk.buildVaultTransaction({ account: account({ timeLock: 1 }), vaultIndex: 0, signers: [noop(A)], instructions: [transferFromVault(await vaultOf())] }),
  },
  {
    code: 'SYNC_NOT_ALLOWED',
    pre: 'buildTransfer: time lock > 0 (errors of buildVaultTransaction)',
    run: () => sdk.buildTransfer({ account: account({ timeLock: 3600 }), vaultIndex: 0, signers: [noop(A)], mint: 'SOL', to: C, amount: 1n, decimals: 9, createRecipientAta: false }),
  },
  {
    code: 'INVALID_SIGNER_SET',
    pre: 'buildSetSigners: duplicate keys in newSigners',
    run: () => sdk.buildSetSigners({ account: account(), signers: [noop(A)], newSigners: [entry(C, 7), entry(C, 7)], newThreshold: 1, rentPayer: noop(X) }),
  },
  {
    code: 'INVALID_SIGNER_SET',
    pre: 'buildSetSigners: newThreshold above the number of voters',
    run: () => sdk.buildSetSigners({ account: account(), signers: [noop(A)], newSigners: [entry(C, 7), entry(X, 5)], newThreshold: 2, rentPayer: noop(X) }),
  },
  {
    code: 'INVALID_SIGNER_SET',
    pre: 'buildCreateAccount: empty signer set',
    run: () => sdk.buildCreateAccount({ ...createBase(), signers: [] }),
  },
  {
    code: 'SIMULATION_FAILED',
    pre: 'send: no computeUnitLimit and the simulation fails',
    run: async () => {
      const payer = await generateKeyPairSigner();
      const { rpc } = makeRpc({
        getLatestBlockhash: () => ({ context: { slot: 1n }, value: { blockhash: blockhash('CmpNeggWJ4JaWJeJ8YKN1Zypmk7uvQq3PECGUCAEMbky'), lastValidBlockHeight: 300n } }),
        getBlockHeight: () => 100n,
        getSlot: () => 1n,
        simulateTransaction: () => ({ context: { slot: 1n }, value: { err: { InstructionError: [0, { Custom: 3 }] }, logs: ['Program log: boom'], accounts: null, unitsConsumed: 1_000n, loadedAccountsDataSize: 1_000, returnData: null } }),
      } as never);
      return sdk.send(rpc, { instructions: [transferFromPayer(payer.address)], feePayer: payer, signers: [payer] });
    },
  },
  {
    code: 'UNSUPPORTED_TRANSACTION',
    pre: 'wrapTransaction: an inner instruction needs a signature the vault cannot give',
    run: async () => {
      const ix: Instruction = { programAddress: SYSTEM_PROGRAM, accounts: [{ address: X, role: AccountRole.WRITABLE_SIGNER }, { address: C, role: AccountRole.WRITABLE }], data: concat(u32(2), u64(1n)) };
      return sdk.wrapTransaction({ account: account(), vaultIndex: 0, signers: [noop(A)], instructions: [transferFromVault(await vaultOf()), ix] });
    },
  },
  {
    code: 'TX_TOO_LARGE',
    pre: 'wrapTransaction: the inner set references more accounts than a v1 transaction can hold',
    run: async () => {
      const vault = await vaultOf();
      const many = Array.from({ length: 70 }, () => ({ address: r.address(), role: AccountRole.WRITABLE }));
      const ix: Instruction = { programAddress: SYSTEM_PROGRAM, accounts: [{ address: vault, role: AccountRole.WRITABLE_SIGNER }, ...many], data: new Uint8Array(1) };
      return sdk.wrapTransaction({ account: account(), vaultIndex: 0, signers: [noop(A)], instructions: [ix] });
    },
  },
  {
    code: 'TX_TOO_LARGE',
    pre: 'buildVaultTransaction: more than 255 inner instructions (plan §5.1)',
    run: async () => {
      const vault = await vaultOf();
      return sdk.buildVaultTransaction({ account: account(), vaultIndex: 0, signers: [noop(A)], instructions: Array.from({ length: 256 }, () => transferFromVault(vault)) });
    },
  },
  {
    code: 'TX_TOO_LARGE',
    pre: 'send: more than 64 addresses',
    run: async () => {
      const payer = await generateKeyPairSigner();
      const accounts = Array.from({ length: 70 }, () => ({ address: r.address(), role: AccountRole.READONLY }));
      const { rpc } = makeRpc({
        getLatestBlockhash: () => ({ context: { slot: 1n }, value: { blockhash: blockhash('CmpNeggWJ4JaWJeJ8YKN1Zypmk7uvQq3PECGUCAEMbky'), lastValidBlockHeight: 300n } }),
        getBlockHeight: () => 100n,
        simulateTransaction: () => ({ context: { slot: 1n }, value: { err: null, logs: [], accounts: null, unitsConsumed: 1_000n, loadedAccountsDataSize: 1_000, returnData: null } }),
      } as never);
      return sdk.send(rpc, { instructions: [{ programAddress: SYSTEM_PROGRAM, accounts, data: new Uint8Array(1) }], feePayer: payer, signers: [payer], computeUnitLimit: 10_000 });
    },
  },
  {
    code: 'SIGNER_MODIFIED_TRANSACTION',
    pre: 'send: a modifying (wallet) signer changes the blockhash',
    run: async () => {
      const payer = await generateKeyPairSigner();
      const wallet: TransactionModifyingSigner = {
        address: payer.address,
        modifyAndSignTransactions: (txs) =>
          Promise.all(txs.map((tx) => {
            const m = decompileTransactionMessage(getCompiledTransactionMessageDecoder().decode(tx.messageBytes));
            const moved = setTransactionMessageLifetimeUsingBlockhash({ blockhash: blockhash('4ruaGCyaofHWGxPFXFVjuEJCdfBGZ2wCtEx6LzdzVqtV'), lastValidBlockHeight: 400n }, m as never);
            return partiallySignTransaction([payer.keyPair], compileTransaction(moved) as never) as never;
          })),
      };
      const { rpc } = makeRpc({
        getLatestBlockhash: () => ({ context: { slot: 1n }, value: { blockhash: blockhash('CmpNeggWJ4JaWJeJ8YKN1Zypmk7uvQq3PECGUCAEMbky'), lastValidBlockHeight: 300n } }),
        sendTransaction: () => assert.fail('nothing may be sent'),
      } as never);
      const ix = { ...transferFromPayer(payer.address), accounts: [{ address: payer.address, role: AccountRole.WRITABLE_SIGNER, signer: wallet }, { address: C, role: AccountRole.WRITABLE }] };
      return sdk.send(rpc, { instructions: [ix], feePayer: wallet, signers: [], computeUnitLimit: 10_000, ...{ loadedAccountsDataSizeLimit: 100_000 } });
    },
  },
  { code: 'INVALID_ARGUMENT', pre: 'deriveVault: vaultIndex 256', run: () => sdk.deriveVault(S, 256) },
  { code: 'INVALID_ARGUMENT', pre: 'deriveVault: vaultIndex -1', run: () => sdk.deriveVault(S, -1) },
  { code: 'INVALID_ARGUMENT', pre: 'deriveVault: non-integer vaultIndex', run: () => sdk.deriveVault(S, 1.5) },
  { code: 'INVALID_ARGUMENT', pre: 'buildCreateAccount: candidateCount 0', run: () => sdk.buildCreateAccount({ ...createBase(), candidateCount: 0 }) },
  { code: 'INVALID_ARGUMENT', pre: 'buildCreateAccount: candidateCount 60 (> 59, v1 address limit)', run: () => sdk.buildCreateAccount({ ...createBase(), candidateCount: 60 }) },
  {
    code: 'INVALID_ARGUMENT',
    pre: 'buildTransfer: amount 0 (plan §5.1 pre: amount > 0)',
    run: () => sdk.buildTransfer({ account: account(), vaultIndex: 0, signers: [noop(A)], mint: 'SOL', to: C, amount: 0n, decimals: 9, createRecipientAta: false }),
  },
];
function transferFromPayer(payer: Address): Instruction {
  return { programAddress: SYSTEM_PROGRAM, accounts: [{ address: payer, role: AccountRole.WRITABLE_SIGNER }, { address: C, role: AccountRole.WRITABLE }], data: concat(u32(2), u64(1n)) };
}

describe('§4 error codes are reachable from documented preconditions', () => {
  it('every §4 code has at least one case', () => {
    assert.deepEqual(ERROR_CODES.filter((c) => !CASES.some((k) => k.code === c)), []);
  });
  for (const c of CASES) it(`${c.code}: ${c.pre}`, () => rejectsWith(c.run, c.code, c.pre));
});

describe('boundaries just inside the preconditions succeed', () => {
  it('deriveVault accepts 0 and 255; buildCreateAccount accepts candidateCount 1 and 59', async () => {
    for (const i of [0, 255]) assert.equal(typeof (await sdk.deriveVault(S, i)), typeof (await sdk.deriveVault(S, 1)));
    for (const n of [1, 59]) assert.equal((await sdk.buildCreateAccount({ ...createBase(), candidateCount: n })).candidates.length, n);
  });
  it('buildVaultTransaction accepts 255 inner instructions', async () => {
    const vault = await vaultOf();
    const ix = await sdk.buildVaultTransaction({ account: account(), vaultIndex: 0, signers: [noop(A)], instructions: Array.from({ length: 255 }, () => transferFromVault(vault)) });
    assert.equal(ix.programAddress, PROGRAM_ID);
  });
});

// ---------- decoder failures surface as SmartAccountError, never a bare RangeError ----------
/** A fetchBalances RPC whose token accounts / mints carry the given raw data. */
function balancesRpc(token: Uint8Array, mint: Uint8Array | null) {
  const b64 = (d: Uint8Array) => [Buffer.from(d).toString('base64'), 'base64'];
  const acct = (d: Uint8Array) => ({ data: b64(d), executable: false, lamports: 1n, owner: TOKEN_PROGRAM, space: BigInt(d.length), rentEpoch: 0n });
  return makeRpc({
    getBalance: () => ({ context: { slot: 1n }, value: 5n }),
    getTokenAccountsByOwner: (_o: string, f: { programId: string }) => ({
      context: { slot: 1n },
      value: f.programId === TOKEN_PROGRAM ? [{ pubkey: X, account: acct(token) }] : [],
    }),
    getMultipleAccounts: (as: string[]) => ({ context: { slot: 1n }, value: as.map(() => (mint === null ? null : acct(mint))) }),
  } as never);
}
const tokenData = (mint: Address) => concat(pubkey(mint), pubkey(S), u64(7n), new Uint8Array(165 - 72));

describe('decoder errors are wrapped', () => {
  it('fetchSpendingLimits: a program account with the SpendingLimit discriminator that does not decode -> NOT_A_SMART_ACCOUNT', async () => {
    const truncated = concat(DISC.spendingLimit, pubkey(S), new Uint8Array(5));
    const { rpc } = accountsRpc(new Map([[X, { data: truncated, owner: PROGRAM_ID }]]));
    await rejectsWith(() => sdk.fetchSpendingLimits(rpc, S), 'NOT_A_SMART_ACCOUNT');
    const raw = await (async () => { try { await sdk.fetchSpendingLimits(rpc, S); } catch (e) { return e; } })();
    assert.ok(!(raw instanceof RangeError), 'not a bare RangeError');
  });
  it('fetchBalances: a token account shorter than 165 bytes -> INVALID_ARGUMENT', async () => {
    await rejectsWith(() => sdk.fetchBalances(balancesRpc(new Uint8Array(100), new Uint8Array(82)).rpc, S), 'INVALID_ARGUMENT');
  });
  it('fetchBalances: a mint shorter than 82 bytes -> INVALID_ARGUMENT; a missing mint -> ACCOUNT_NOT_FOUND (never decimals 0)', async () => {
    await rejectsWith(() => sdk.fetchBalances(balancesRpc(tokenData(C), new Uint8Array(40)).rpc, S), 'INVALID_ARGUMENT');
    await rejectsWith(() => sdk.fetchBalances(balancesRpc(tokenData(C), null).rpc, S), 'ACCOUNT_NOT_FOUND');
  });
  it('fetchBalances: well-formed data decodes (the error cases above are not over-eager)', async () => {
    const mint = new Uint8Array(82);
    mint[44] = 6;
    const b = await sdk.fetchBalances(balancesRpc(tokenData(C), mint).rpc, S);
    assert.deepEqual(b.tokens.map((t) => [t.mint, t.amount, t.decimals]), [[C, 7n, 6]]);
  });
});
