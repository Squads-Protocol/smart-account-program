// G-v1 signer check: can a given signer sign a Solana v1 (SIMD-0385) transaction?
//
//   node scripts/g-v1-signer-check.mjs build  --payer <address> [--rpc <url>]
//   node scripts/g-v1-signer-check.mjs attach --tx <base64 wire tx> --sig <base58|hex signature> [--signer <address>]
//   node scripts/g-v1-signer-check.mjs submit --tx <base64 signed wire tx> [--rpc <url>]
//   node scripts/g-v1-signer-check.mjs selftest
//
// build:  a v1 transaction with one Memo instruction signed by --payer (who also pays ~5,000 lamports).
//         Prints the unsigned wire transaction (for wallet / Privy signTransaction) and the message
//         bytes in hex (for raw-payload signers such as Turnkey sign_raw_payload).
// attach: puts a raw 64-byte signature into the wire transaction (for raw-payload signers).
// submit: verifies every signature locally, sends, and waits for confirmation or blockhash expiry.
// selftest: offline round trip with a generated key; checks the script itself, sends nothing.

import {
  address,
  appendTransactionMessageInstruction,
  compileTransaction,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createTransactionMessage,
  getBase58Decoder,
  getBase58Encoder,
  getBase64EncodedWireTransaction,
  getBase64Encoder,
  getPublicKeyFromAddress,
  getTransactionDecoder,
  pipe,
  setTransactionMessageComputeUnitLimit,
  setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
  setTransactionMessageLoadedAccountsDataSizeLimit,
  signTransaction,
  verifySignature,
} from '@solana/kit';

const MEMO_PROGRAM = address('MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr');
const DEFAULT_RPC = process.env.SOLANA_RPC_URL ?? 'https://api.mainnet-beta.solana.com';
const COMPUTE_UNIT_LIMIT = 50_000;
const LOADED_ACCOUNTS_DATA_SIZE = 1024 * 1024;

function args() {
  const [, , cmd, ...rest] = process.argv;
  const opts = {};
  for (let i = 0; i < rest.length; i += 2) opts[rest[i].replace(/^--/, '')] = rest[i + 1];
  return { cmd, opts };
}

function need(opts, key) {
  if (!opts[key]) throw new Error(`missing --${key}`);
  return opts[key];
}

function buildMessage(payer, blockhash, lastValidBlockHeight) {
  const memo = {
    programAddress: MEMO_PROGRAM,
    accounts: [{ address: payer, role: 2 /* readonly signer */ }],
    data: new TextEncoder().encode('squads g-v1 signer check'),
  };
  return pipe(
    createTransactionMessage({ version: 1 }),
    (m) => setTransactionMessageFeePayer(payer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash({ blockhash, lastValidBlockHeight }, m),
    (m) => appendTransactionMessageInstruction(memo, m),
    (m) => setTransactionMessageComputeUnitLimit(COMPUTE_UNIT_LIMIT, m),
    (m) => setTransactionMessageLoadedAccountsDataSizeLimit(LOADED_ACCOUNTS_DATA_SIZE, m),
  );
}

function decodeWire(b64) {
  return getTransactionDecoder().decode(getBase64Encoder().encode(b64));
}

function parseSignature(sig) {
  const bytes = /^[0-9a-fA-F]{128}$/.test(sig) ? Uint8Array.from(Buffer.from(sig, 'hex')) : getBase58Encoder().encode(sig);
  if (bytes.length !== 64) throw new Error(`signature must be 64 bytes, got ${bytes.length}`);
  return bytes;
}

async function verifyAll(tx) {
  if (tx.messageBytes[0] !== 0x81) throw new Error(`not a v1 transaction (first message byte 0x${tx.messageBytes[0].toString(16)})`);
  const results = [];
  for (const [signer, sig] of Object.entries(tx.signatures)) {
    if (!sig) {
      results.push({ signer, ok: false, reason: 'missing' });
      continue;
    }
    const ok = await verifySignature(await getPublicKeyFromAddress(address(signer)), sig, tx.messageBytes);
    results.push({ signer, ok });
  }
  return results;
}

async function cmdBuild(opts) {
  const payer = address(need(opts, 'payer'));
  const rpc = createSolanaRpc(opts.rpc ?? DEFAULT_RPC);
  const { value } = await rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
  const tx = compileTransaction(buildMessage(payer, value.blockhash, value.lastValidBlockHeight));
  console.log(
    JSON.stringify(
      {
        payer,
        wireTransactionBase64: getBase64EncodedWireTransaction(tx),
        messageHex: Buffer.from(tx.messageBytes).toString('hex'),
        lastValidBlockHeight: String(value.lastValidBlockHeight),
        note: 'Sign within ~60 seconds, before the blockhash expires.',
      },
      null,
      2,
    ),
  );
}

async function cmdAttach(opts) {
  const tx = decodeWire(need(opts, 'tx'));
  const signers = Object.keys(tx.signatures);
  const signer = opts.signer ?? signers[0];
  if (!signers.includes(signer)) throw new Error(`${signer} is not a required signer (${signers.join(', ')})`);
  const signed = { ...tx, signatures: { ...tx.signatures, [signer]: parseSignature(need(opts, 'sig')) } };
  console.log(getBase64EncodedWireTransaction(signed));
}

async function cmdSubmit(opts) {
  const tx = decodeWire(need(opts, 'tx'));
  const results = await verifyAll(tx);
  console.log(JSON.stringify({ verify: results }));
  if (results.some((r) => !r.ok)) throw new Error('signature verification failed; not sending');
  const rpc = createSolanaRpc(opts.rpc ?? DEFAULT_RPC);
  const wire = getBase64EncodedWireTransaction(tx);
  const signature = getBase58Decoder().decode(Object.values(tx.signatures)[0]);
  await rpc.sendTransaction(wire, { encoding: 'base64', skipPreflight: false, preflightCommitment: 'confirmed' }).send();
  const lastValid = BigInt(tx.lifetimeConstraint?.lastValidBlockHeight ?? 0n);
  for (;;) {
    const { value } = await rpc.getSignatureStatuses([signature]).send();
    const status = value[0];
    if (status?.err) throw new Error(`landed with error: ${JSON.stringify(status.err)}`);
    if (status?.confirmationStatus === 'confirmed' || status?.confirmationStatus === 'finalized') {
      console.log(JSON.stringify({ result: 'PASS', signature }));
      return;
    }
    const height = await rpc.getBlockHeight({ commitment: 'confirmed' }).send();
    if (lastValid > 0n && height > lastValid) throw new Error(`expired without landing (signature ${signature})`);
    await new Promise((r) => setTimeout(r, 1500));
  }
}

async function cmdSelftest() {
  const seed = new Uint8Array(32).fill(7);
  const { webcrypto } = await import('node:crypto');
  const pkcs8 = new Uint8Array([0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20, ...seed]);
  const priv = await webcrypto.subtle.importKey('pkcs8', pkcs8, 'Ed25519', true, ['sign']);
  const jwk = await webcrypto.subtle.exportKey('jwk', priv);
  const pub = Buffer.from(jwk.x, 'base64url');
  const signer = await createKeyPairSignerFromBytes(new Uint8Array([...seed, ...pub]));
  const blockhash = '11111111111111111111111111111111';
  const tx = compileTransaction(buildMessage(signer.address, blockhash, 1n));
  if (tx.messageBytes[0] !== 0x81) throw new Error('selftest: message is not v1');
  // Path 1: whole-transaction signing (wallets, Privy signTransaction).
  const signedWhole = await signTransaction([signer.keyPair], tx);
  const r1 = await verifyAll(decodeWire(getBase64EncodedWireTransaction(signedWhole)));
  // Path 2: raw-payload signing over messageHex (Turnkey), then attach.
  const raw = new Uint8Array(await webcrypto.subtle.sign('Ed25519', priv, tx.messageBytes));
  const attached = { ...tx, signatures: { ...tx.signatures, [signer.address]: raw } };
  const r2 = await verifyAll(decodeWire(getBase64EncodedWireTransaction(attached)));
  const ok = r1.every((r) => r.ok) && r2.every((r) => r.ok);
  console.log(JSON.stringify({ selftest: ok ? 'PASS' : 'FAIL', wholeTx: r1, rawPayload: r2, wireBytes: getBase64Encoder().encode(getBase64EncodedWireTransaction(tx)).length }));
  if (!ok) process.exit(1);
}

const { cmd, opts } = args();
const commands = { build: cmdBuild, attach: cmdAttach, submit: cmdSubmit, selftest: cmdSelftest };
if (!commands[cmd]) {
  console.error('usage: g-v1-signer-check.mjs build|attach|submit|selftest [options]');
  process.exit(2);
}
commands[cmd](opts).catch((e) => {
  console.error(`FAIL: ${e.message}`);
  process.exit(1);
});
