// Live end-to-end check of the SDK on devnet (or any cluster): real v1 sends, not simulation.
//
//   E2E_PAYER_KEYPAIR=path/to/keypair.json [SOLANA_RPC_URL=https://api.devnet.solana.com] npm run e2e:devnet
//
// The keypair file is a JSON array of 64 bytes (solana-keygen format) holding about 0.4 SOL. It becomes the only
// signer of a fresh smart account, then exercises: account creation (25 candidates) + resolve + read; wrapping a
// customer-built base64 v0 transaction; an SPL transferChecked with recipient ATA creation + fetchBalances; signer
// changes (1-of-1 → 2-of-2 → 1-of-1); and listing + removing a spending limit. Results go to stdout as JSON lines
// and to ./e2e-devnet-results.json. Not part of `npm test` or CI: it spends real SOL and needs a live RPC.
import { readFileSync, writeFileSync } from 'node:fs';
import {
  AccountRole,
  address,
  appendTransactionMessageInstructions,
  compileTransaction,
  createKeyPairSignerFromBytes,
  createDefaultRpcTransport,
  createSolanaRpcFromTransport,
  createTransactionMessage,
  decompileTransactionMessage,
  fetchEncodedAccount,
  generateKeyPairSigner,
  getBase64EncodedWireTransaction,
  getBase64Encoder,
  getCompiledTransactionMessageDecoder,
  getTransactionDecoder,
  type Address,
  type Instruction,
  type KeyPairSigner,
  type Signature,
  pipe,
  setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
} from '@solana/kit';
import * as sdk from '../src/index.ts';
import * as h from '../test/support/program.ts';

const keypairPath = process.env.E2E_PAYER_KEYPAIR;
if (!keypairPath) {
  console.error('Set E2E_PAYER_KEYPAIR to a funded keypair file (JSON array of 64 bytes).');
  process.exit(2);
}
// Public RPCs rate-limit (HTTP 429); retry those with backoff so a run does not fail on the free endpoint.
const transport = createDefaultRpcTransport({ url: process.env.SOLANA_RPC_URL ?? 'https://api.devnet.solana.com' });
const rpc = createSolanaRpcFromTransport(async <T>(config: Parameters<typeof transport>[0]): Promise<T> => {
  for (let attempt = 0; ; attempt++) {
    try {
      return await transport<T>(config);
    } catch (e) {
      if (attempt >= 8 || !String(e instanceof Error ? e.message : e).includes('429')) throw e;
      await new Promise((r) => setTimeout(r, 500 * 2 ** attempt));
    }
  }
});
const payer = await createKeyPairSignerFromBytes(new Uint8Array(JSON.parse(readFileSync(keypairPath, 'utf8')) as number[]));
const ALL: sdk.Permission[] = ['initiate', 'vote', 'execute'];
const MEMO = address('MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr');

interface Result { step: string; ok: boolean; signature?: string; check: string }
const results: Result[] = [];

const balance = async (a: Address) => (await rpc.getBalance(a, { commitment: 'confirmed' }).send()).value;
const signerMeta = (s: KeyPairSigner, writable: boolean) => ({ address: s.address, role: writable ? AccountRole.WRITABLE_SIGNER : AccountRole.READONLY_SIGNER, signer: s });
const transfer = (from: KeyPairSigner, to: Address, lamports: bigint): Instruction => ({
  programAddress: h.SYSTEM_PROGRAM,
  accounts: [signerMeta(from, true), { address: to, role: AccountRole.WRITABLE }],
  data: h.concat(h.u32(2), h.u64(lamports)),
});
async function sendOk(input: sdk.SendInput): Promise<Signature> {
  const outcome = await sdk.send(rpc, input);
  if (outcome.status !== 'confirmed') throw new Error(`send returned ${outcome.status}`);
  return outcome.signature;
}
async function step(name: string, fn: () => Promise<{ signature?: string; check: string }>): Promise<boolean> {
  try {
    const r = await fn();
    results.push({ step: name, ok: true, ...r });
    console.log(JSON.stringify({ step: name, ok: true, ...r }));
    return true;
  } catch (e) {
    const check = e instanceof Error ? e.message : String(e);
    results.push({ step: name, ok: false, check });
    console.log(JSON.stringify({ step: name, ok: false, error: check }));
    return false;
  }
}

let settings: Address | undefined;
let vault: sdk.VaultAddress | undefined;
const recipient = (await generateKeyPairSigner()).address;

await step('create account (25 candidates) + resolveCreatedAccount + fetchAccount', async () => {
  const pc = await fetchEncodedAccount(rpc, await sdk.deriveProgramConfig(), { commitment: 'confirmed' });
  if (!pc.exists) throw new Error('ProgramConfig not found');
  const built = await sdk.buildCreateAccount({ programConfig: { data: pc.data }, creator: payer, signers: [{ key: payer.address, permissions: ALL }], threshold: 1 });
  const signature = await sendOk({ instructions: [built.instruction], feePayer: payer });
  settings = await sdk.resolveCreatedAccount(rpc, signature, built.candidates);
  vault = await sdk.deriveVault(settings, 0);
  const account = await sdk.fetchAccount(rpc, settings);
  if (account.signers.length !== 1 || account.signers[0]?.key !== payer.address || account.threshold !== 1) throw new Error('unexpected signer set');
  await sendOk({ instructions: [transfer(payer, vault, 60_000_000n)], feePayer: payer });
  return { signature, check: `settings ${settings}, ${built.candidates.length} candidates, 1 signer, vault funded with 0.06 SOL` };
});
if (!settings || !vault) {
  writeFileSync('e2e-devnet-results.json', JSON.stringify(results, null, 2));
  process.exit(1);
}
const S = settings;
const V = vault;

await step('wrapTransaction of a customer-built base64 v0 transaction (memo + SOL transfer)', async () => {
  const { value: latest } = await rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
  const v0 = pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayer(V, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(latest, m),
    (m) => appendTransactionMessageInstructions([
      { programAddress: MEMO, accounts: [], data: new TextEncoder().encode('smart-account-core e2e') },
      { programAddress: h.SYSTEM_PROGRAM, accounts: [{ address: V, role: AccountRole.WRITABLE_SIGNER }, { address: recipient, role: AccountRole.WRITABLE }], data: h.concat(h.u32(2), h.u64(2_000_000n)) },
    ], m),
  );
  const base64 = getBase64EncodedWireTransaction(compileTransaction(v0));
  const tx = getTransactionDecoder().decode(getBase64Encoder().encode(base64));
  const message = decompileTransactionMessage(getCompiledTransactionMessageDecoder().decode(tx.messageBytes));
  const wrapped = await sdk.wrapTransaction({ account: await sdk.fetchAccount(rpc, S), vaultIndex: 0, signers: [payer], message });
  const signature = await sendOk({ instructions: [wrapped.instruction], feePayer: payer });
  const got = await balance(recipient);
  if (got !== 2_000_000n) throw new Error(`recipient holds ${got} lamports`);
  return { signature, check: 'recipient received 2,000,000 lamports from the vault' };
});

await step('buildTransfer SPL (transferChecked, recipient ATA created by the vault) + fetchBalances', async () => {
  const mint = await generateKeyPairSigner();
  const rent = await rpc.getMinimumBalanceForRentExemption(82n).send();
  const vaultAta = await h.ataPda(V, mint.address);
  await sendOk({
    feePayer: payer,
    instructions: [
      { programAddress: h.SYSTEM_PROGRAM, accounts: [signerMeta(payer, true), signerMeta(mint, true)], data: h.concat(h.u32(0), h.u64(rent), h.u64(82n), h.pubkey(h.TOKEN_PROGRAM)) },
      { programAddress: h.TOKEN_PROGRAM, accounts: [{ address: mint.address, role: AccountRole.WRITABLE }], data: h.concat(h.u8(20), h.u8(6), h.pubkey(payer.address), h.u8(0)) },
      { programAddress: h.ATA_PROGRAM, accounts: [signerMeta(payer, true), { address: vaultAta, role: AccountRole.WRITABLE }, { address: V, role: AccountRole.READONLY }, { address: mint.address, role: AccountRole.READONLY }, { address: h.SYSTEM_PROGRAM, role: AccountRole.READONLY }, { address: h.TOKEN_PROGRAM, role: AccountRole.READONLY }], data: h.u8(1) },
      { programAddress: h.TOKEN_PROGRAM, accounts: [{ address: mint.address, role: AccountRole.WRITABLE }, { address: vaultAta, role: AccountRole.WRITABLE }, signerMeta(payer, false)], data: h.concat(h.u8(7), h.u64(2_500_000n)) },
    ],
  });
  const to = (await generateKeyPairSigner()).address;
  const ix = await sdk.buildTransfer({ account: await sdk.fetchAccount(rpc, S), vaultIndex: 0, signers: [payer], mint: mint.address, to, amount: 1_500_000n, decimals: 6, createRecipientAta: true });
  const signature = await sendOk({ instructions: [ix], feePayer: payer });
  const received = (await rpc.getTokenAccountBalance(await h.ataPda(to, mint.address), { commitment: 'confirmed' }).send()).value.amount;
  const vaultTokens = (await sdk.fetchBalances(rpc, V)).tokens.find((t) => t.mint === mint.address)?.amount;
  if (received !== '1500000' || vaultTokens !== 1_000_000n) throw new Error(`recipient ${received}, vault ${String(vaultTokens)}`);
  return { signature, check: 'recipient ATA created and holds 1.5 tokens; fetchBalances shows 1.0 left in the vault' };
});

const second = await generateKeyPairSigner();
await step('buildSetSigners: 1-of-1 → 2-of-2', async () => {
  const ix = sdk.buildSetSigners({ account: await sdk.fetchAccount(rpc, S), signers: [payer], newSigners: [{ key: payer.address, permissions: ALL }, { key: second.address, permissions: ALL }], newThreshold: 2, rentPayer: payer });
  const signature = await sendOk({ instructions: [ix], feePayer: payer });
  const after = await sdk.fetchAccount(rpc, S);
  if (after.signers.length !== 2 || after.threshold !== 2) throw new Error(`${after.signers.length} signers, threshold ${after.threshold}`);
  return { signature, check: '2 signers, threshold 2' };
});
await step('buildSetSigners: 2-of-2 → 1-of-1 (both keys sign)', async () => {
  const ix = sdk.buildSetSigners({ account: await sdk.fetchAccount(rpc, S), signers: [payer, second], newSigners: [{ key: payer.address, permissions: ALL }], newThreshold: 1, rentPayer: payer });
  const signature = await sendOk({ instructions: [ix], feePayer: payer, signers: [second] });
  const after = await sdk.fetchAccount(rpc, S);
  if (after.signers.length !== 1 || after.threshold !== 1) throw new Error(`${after.signers.length} signers, threshold ${after.threshold}`);
  return { signature, check: '1 signer, threshold 1' };
});

await step('fetchSpendingLimits + buildRemoveSpendingLimit', async () => {
  const seed = (await generateKeyPairSigner()).address;
  const spender = (await generateKeyPairSigner()).address;
  const limit = await h.spendingLimitPda(S, seed);
  const add = h.addSpendingLimitAction({ seed, accountIndex: 0, mint: h.DEFAULT_PUBKEY, amount: 1_000_000n, period: 3, signers: [spender], destinations: [recipient], expiration: 9223372036854775807n });
  await sendOk({ instructions: [h.settingsSyncIx({ settings: S, rentPayer: payer, signers: [payer], actions: [add], remaining: [limit] })], feePayer: payer });
  const listed = await sdk.fetchSpendingLimits(rpc, S);
  if (listed.length !== 1 || listed[0]?.address !== limit) throw new Error(`listed ${listed.length}`);
  const ix = sdk.buildRemoveSpendingLimit({ account: await sdk.fetchAccount(rpc, S), signers: [payer], spendingLimit: limit, rentPayer: payer });
  const signature = await sendOk({ instructions: [ix], feePayer: payer });
  const left = await sdk.fetchSpendingLimits(rpc, S);
  if (left.length !== 0) throw new Error(`${left.length} limits left`);
  return { signature, check: 'limit listed, then removed; 0 left' };
});

writeFileSync('e2e-devnet-results.json', JSON.stringify(results, null, 2));
const passed = results.filter((r) => r.ok).length;
console.log(JSON.stringify({ summary: `${passed}/${results.length} passed` }));
process.exit(passed === results.length ? 0 : 1);
