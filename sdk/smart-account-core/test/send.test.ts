// send / checkOutcome contract (spec §5 "Send", plan §3.3) against a scripted fake RPC.
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  AccountRole,
  type AccountSignerMeta,
  type Address,
  type Instruction,
  type KeyPairSigner,
  type Signature,
  type TransactionModifyingSigner,
  appendTransactionMessageInstructions,
  compileTransaction,
  blockhash,
  createTransactionMessage,
  decompileTransactionMessage,
  generateKeyPairSigner,
  getBase64EncodedWireTransaction,
  getCompiledTransactionMessageDecoder,
  getSignatureFromTransaction,
  getTransactionDecoder,
  partiallySignTransaction,
  pipe,
  setTransactionMessageComputeUnitLimit,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  setTransactionMessageLoadedAccountsDataSizeLimit,
  signTransactionMessageWithSigners,
} from '@solana/kit';

import { SYSTEM_PROGRAM, concat, u32, u64 } from './support/program.ts';
import { cfg, makeRpc, wireBytes } from './support/rpc.ts';
import { type CheckedOutcome, sdk } from './support/sdk.ts';
import { rejectsWith, show } from './support/util.ts';

const BLOCKHASHES = [
  blockhash('CmpNeggWJ4JaWJeJ8YKN1Zypmk7uvQq3PECGUCAEMbky'),
  blockhash('4ruaGCyaofHWGxPFXFVjuEJCdfBGZ2wCtEx6LzdzVqtV'),
  blockhash('EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG'),
];
const MAX_CU = 1_400_000;
const MAX_LOADED = 64 * 1024 * 1024;

interface Script {
  /** status returned for a signature that was sent; `undefined` = not found */
  status?: (n: number) => { err: unknown } | undefined;
  sendThrows?: boolean;
  simulateErr?: unknown;
  heightStep?: bigint;
}
const decodeWire = (bytes: Uint8Array) => {
  const tx = getTransactionDecoder().decode(bytes);
  const message = decompileTransactionMessage(getCompiledTransactionMessageDecoder().decode(tx.messageBytes));
  return { tx, message, signature: getSignatureFromTransaction(tx) };
};

/** A scripted RPC for `send`. Block height starts at 100 and moves `heightStep` per height read. */
function sendRpc(s: Script = {}) {
  let height = 100n;
  let blockhashCalls = 0;
  let statusPolls = 0;
  const sent: Uint8Array[] = [];
  const lvbh = (k: number) => 250n + BigInt(k) * 1_000n;
  const readHeight = () => {
    const h = height;
    height += s.heightStep ?? 0n;
    return h;
  };
  const sentSigs = () => sent.map((b) => decodeWire(b).signature);
  const net = makeRpc({
    getLatestBlockhash: () => {
      const k = blockhashCalls++;
      return { context: { slot: 5n }, value: { blockhash: BLOCKHASHES[k % 3]!, lastValidBlockHeight: lvbh(k) } };
    },
    getBlockHeight: readHeight,
    getEpochInfo: () => ({ absoluteSlot: 5n, blockHeight: readHeight(), epoch: 0n, slotIndex: 5n, slotsInEpoch: 432_000n, transactionCount: null }),
    getSlot: () => 5n,
    simulateTransaction: () => ({
      context: { slot: 5n },
      value: {
        err: s.simulateErr ?? null,
        logs: [],
        accounts: null,
        unitsConsumed: 42_000n,
        loadedAccountsDataSize: 70_000,
        returnData: null,
        innerInstructions: null,
        replacementBlockhash: null,
      },
    }),
    sendTransaction: (wire: string, c: unknown) => {
      const bytes = wireBytes(wire, c);
      sent.push(bytes);
      if (s.sendThrows) throw new Error('fetch failed: socket hang up (simulated timeout)');
      return decodeWire(bytes).signature;
    },
    getSignatureStatuses: (sigs: string[]) => ({
      context: { slot: 6n },
      value: sigs.map((sig) => {
        if (!sentSigs().includes(sig as Signature)) return null;
        const st = s.status?.(statusPolls++);
        return st === undefined ? null : { slot: 9n, confirmations: null, err: st.err, confirmationStatus: 'confirmed', status: st.err ? { Err: st.err } : { Ok: null } };
      }),
    }),
    getTransaction: (sig: string, c: unknown) => {
      const st = sentSigs().includes(sig as Signature) ? s.status?.(statusPolls) : undefined;
      if (st === undefined) return null;
      void c;
      return { slot: 9n, blockTime: null, version: 1, meta: { err: st.err, logMessages: ['Program log: AnchorError ... Error Number: 6005.'], fee: 5000n, preBalances: [], postBalances: [] }, transaction: { signatures: [sig], message: { accountKeys: [], instructions: [], recentBlockhash: '' } } };
    },
  } as never);
  return { ...net, sent, lvbh, blockhashCalls: () => blockhashCalls };
}

const transferIx = (from: KeyPairSigner, to: Address, n = 1n): Instruction => ({
  programAddress: SYSTEM_PROGRAM,
  accounts: [
    { address: from.address, role: AccountRole.WRITABLE_SIGNER, signer: from } as AccountSignerMeta,
    { address: to, role: AccountRole.WRITABLE },
  ],
  data: concat(u32(2), u64(n)),
});
async function basic() {
  const payer = await generateKeyPairSigner();
  const to = (await generateKeyPairSigner()).address;
  return { payer, ixs: [transferIx(payer, to)] };
}
const sentConfig = (bytes: Uint8Array) => {
  const { message } = decodeWire(bytes);
  return (message as { config?: { computeUnitLimit?: number; loadedAccountsDataSizeLimit?: number; priorityFeeLamports?: bigint } }).config ?? {};
};
function assertSentOnce(r: ReturnType<typeof sendRpc>) {
  assert.equal(r.count('sendTransaction'), 1, 'sendTransaction called exactly once');
  assert.equal(r.blockhashCalls(), 1, 'one blockhash: the transaction is never rebuilt');
}

describe('send', () => {
  it('confirmed: returns this transaction\'s signature and slot; v1 wire bytes (0x81)', async () => {
    const r = sendRpc({ status: () => ({ err: null }) });
    const { payer, ixs } = await basic();
    const out = await sdk.send(r.rpc, { instructions: ixs, feePayer: payer, signers: [payer] });
    assert.equal(out.status, 'confirmed', show(out));
    assertSentOnce(r);
    const bytes = r.sent[0]!;
    assert.equal(bytes[0], 0x81, 'v1 wire format: version byte 0x81 at offset 0');
    assert.equal(decodeWire(bytes).message.version, 1);
    assert.equal(out.signature, decodeWire(bytes).signature);
    if (out.status === 'confirmed') assert.equal(out.slot, 9n);
  });

  it('sets CU limit and loaded-accounts size in the v1 message config when the caller omits them (simulation path)', async () => {
    const r = sendRpc({ status: () => ({ err: null }) });
    const { payer, ixs } = await basic();
    await sdk.send(r.rpc, { instructions: ixs, feePayer: payer, signers: [payer] });
    assert.ok(r.count('simulateTransaction') >= 1, 'simulated first');
    const simIdx = r.calls.findIndex((c) => c.method === 'simulateTransaction');
    const sendIdx = r.calls.findIndex((c) => c.method === 'sendTransaction');
    assert.ok(simIdx < sendIdx, 'simulation happens before sending');
    const c = sentConfig(r.sent[0]!);
    assert.ok(c.computeUnitLimit !== undefined && c.computeUnitLimit >= 42_000 && c.computeUnitLimit <= MAX_CU, `CU limit ${c.computeUnitLimit}`);
    assert.ok(
      c.loadedAccountsDataSizeLimit !== undefined && c.loadedAccountsDataSizeLimit >= 70_000 && c.loadedAccountsDataSizeLimit <= MAX_LOADED,
      `loaded-accounts limit ${c.loadedAccountsDataSizeLimit}`,
    );
    assert.equal(decodeWire(r.sent[0]!).message.instructions.length, 1, 'no ComputeBudget instructions added (v1 uses the config)');
  });

  it('uses the caller\'s computeUnitLimit and priorityFeeLamports exactly, and still sets loaded-accounts size', async () => {
    const r = sendRpc({ status: () => ({ err: null }) });
    const { payer, ixs } = await basic();
    await sdk.send(r.rpc, { instructions: ixs, feePayer: payer, signers: [payer], computeUnitLimit: 250_000, priorityFeeLamports: 777n });
    const c = sentConfig(r.sent[0]!);
    assert.equal(c.computeUnitLimit, 250_000);
    assert.equal(c.priorityFeeLamports, 777n);
    assert.ok((c.loadedAccountsDataSizeLimit ?? 0) > 0, 'loaded-accounts size is never left at 0');
  });

  it('failed: a program error is returned as failed with the signature and the error', async () => {
    const err = { InstructionError: [0, { Custom: 6005 }] };
    const r = sendRpc({ status: () => ({ err }) });
    const { payer, ixs } = await basic();
    const out = await sdk.send(r.rpc, { instructions: ixs, feePayer: payer, signers: [payer] });
    assert.equal(out.status, 'failed', show(out));
    assert.equal(out.signature, decodeWire(r.sent[0]!).signature);
    if (out.status === 'failed') assert.match(show(out.error), /6005/, 'error info carries the program error code');
    assertSentOnce(r);
  });

  it('timeout (send RPC errors after signing) -> unknown with signature and lastValidBlockHeight; nothing re-sent', async () => {
    const r = sendRpc({ sendThrows: true, heightStep: 60n });
    const { payer, ixs } = await basic();
    const out = await sdk.send(r.rpc, { instructions: ixs, feePayer: payer, signers: [payer] });
    assert.equal(out.status, 'unknown', show(out));
    assert.equal(out.signature, decodeWire(r.sent[0]!).signature, 'the signature of the one signed transaction');
    if (out.status === 'unknown') assert.equal(out.lastValidBlockHeight, r.lvbh(0));
    assertSentOnce(r);
  });

  it('a sendTransaction error is carried as `cause` on unknown, and nothing is re-sent', async () => {
    const r = sendRpc({ sendThrows: true, heightStep: 60n });
    const { payer, ixs } = await basic();
    const out = await sdk.send(r.rpc, { instructions: ixs, feePayer: payer, signers: [payer] });
    assert.equal(out.status, 'unknown', show(out));
    if (out.status !== 'unknown') return;
    assert.ok(out.cause instanceof Error, `cause is the RPC error: ${show(out)}`);
    assert.match(out.cause.message, /socket hang up/);
    assertSentOnce(r);
    assert.equal(r.count('getSignatureStatuses'), 0, 'send returns at once; resolving is checkOutcome\'s job');
  });

  it('an unknown that is not caused by an RPC error has no cause', async () => {
    const r = sendRpc({ heightStep: 400n });
    const { payer, ixs } = await basic();
    const out = await sdk.send(r.rpc, { instructions: ixs, feePayer: payer, signers: [payer] });
    assert.equal(out.status, 'unknown', show(out));
    assert.equal('cause' in out, false, show(out));
  });

  it('never confirmed before expiry -> unknown (not failed), confirmed against its own lastValidBlockHeight', async () => {
    // A second getLatestBlockhash would return a later lastValidBlockHeight; the SDK must not use it.
    const r = sendRpc({ heightStep: 40n });
    const { payer, ixs } = await basic();
    const out = await sdk.send(r.rpc, { instructions: ixs, feePayer: payer, signers: [payer] });
    assert.equal(out.status, 'unknown', show(out));
    if (out.status === 'unknown') assert.equal(out.lastValidBlockHeight, r.lvbh(0));
    assertSentOnce(r);
  });

  it('a failing simulation throws SIMULATION_FAILED and sends nothing', async () => {
    const r = sendRpc({ simulateErr: { InstructionError: [0, { Custom: 1 }] } });
    const { payer, ixs } = await basic();
    await rejectsWith(() => sdk.send(r.rpc, { instructions: ixs, feePayer: payer, signers: [payer] }), 'SIMULATION_FAILED');
    assert.equal(r.count('sendTransaction'), 0);
  });

  describe('v1 limits -> TX_TOO_LARGE, nothing sent', () => {
    const withAccounts = async (n: number) => {
      const payer = await generateKeyPairSigner();
      const accounts = await Promise.all(Array.from({ length: n }, async () => ({ address: (await generateKeyPairSigner()).address, role: AccountRole.READONLY })));
      return { payer, ixs: [{ programAddress: SYSTEM_PROGRAM, accounts, data: new Uint8Array(1) }] };
    };
    const withSigners = async (n: number) => {
      const payer = await generateKeyPairSigner();
      const extra = await Promise.all(Array.from({ length: n - 1 }, () => generateKeyPairSigner()));
      const accounts = extra.map((s): AccountSignerMeta => ({ address: s.address, role: AccountRole.READONLY_SIGNER, signer: s }));
      return { payer, extra, ixs: [{ programAddress: SYSTEM_PROGRAM, accounts, data: new Uint8Array(1) }] };
    };
    it('65 addresses is too large; 64 is accepted', async () => {
      const big = await withAccounts(63); // + fee payer + program = 65
      const r1 = sendRpc({ status: () => ({ err: null }) });
      await rejectsWith(() => sdk.send(r1.rpc, { instructions: big.ixs, feePayer: big.payer, signers: [big.payer] }), 'TX_TOO_LARGE', '65 addresses');
      assert.equal(r1.count('sendTransaction'), 0);
      const ok = await withAccounts(62);
      const r2 = sendRpc({ status: () => ({ err: null }) });
      const out = await sdk.send(r2.rpc, { instructions: ok.ixs, feePayer: ok.payer, signers: [ok.payer] });
      assert.equal(out.status, 'confirmed', '64 addresses fit');
    });
    it('13 signatures is too large; 12 is accepted', async () => {
      const big = await withSigners(13);
      const r1 = sendRpc({ status: () => ({ err: null }) });
      await rejectsWith(() => sdk.send(r1.rpc, { instructions: big.ixs, feePayer: big.payer, signers: [big.payer, ...big.extra] }), 'TX_TOO_LARGE', '13 signers');
      assert.equal(r1.count('sendTransaction'), 0);
      const ok = await withSigners(12);
      const r2 = sendRpc({ status: () => ({ err: null }) });
      const out = await sdk.send(r2.rpc, { instructions: ok.ixs, feePayer: ok.payer, signers: [ok.payer, ...ok.extra] });
      assert.equal(out.status, 'confirmed', '12 signers fit');
    });
    it('more than 4096 bytes is too large', async () => {
      const payer = await generateKeyPairSigner();
      const r = sendRpc({ status: () => ({ err: null }) });
      const ixs: Instruction[] = [{ programAddress: SYSTEM_PROGRAM, accounts: [], data: new Uint8Array(4_200) }];
      await rejectsWith(() => sdk.send(r.rpc, { instructions: ixs, feePayer: payer, signers: [payer] }), 'TX_TOO_LARGE', '4200-byte data');
      assert.equal(r.count('sendTransaction'), 0);
    });
  });
});

// ---------- wallet-standard (modifying) signers ----------
type Tx = Parameters<TransactionModifyingSigner['modifyAndSignTransactions']>[0][number];
/** A modifying signer for `kp` that rewrites the message with `edit` (identity when omitted) before signing. */
function modifyingSigner(kp: KeyPairSigner, edit?: (m: ReturnType<typeof decodeWire>['message']) => Parameters<typeof compileTransaction>[0]) {
  const signer: TransactionModifyingSigner = {
    address: kp.address,
    async modifyAndSignTransactions(txs) {
      return Promise.all(
        txs.map(async (tx: Tx) => {
          const base = edit
            ? compileTransaction(edit(decompileTransactionMessage(getCompiledTransactionMessageDecoder().decode(tx.messageBytes))))
            : tx;
          return (await partiallySignTransaction([kp.keyPair], base as never)) as never;
        }),
      );
    },
  };
  return signer;
}
/** Safeguard removed: a send that signs and sends without comparing the signed message to the compiled one. */
async function uncheckedSend(rpc: ReturnType<typeof sendRpc>['rpc'], payer: TransactionModifyingSigner, ixs: Instruction[]) {
  const r = rpc as unknown as Record<string, (...a: unknown[]) => { send(): Promise<never> }>;
  const { value } = (await r.getLatestBlockhash!().send()) as { value: { blockhash: never; lastValidBlockHeight: bigint } };
  const msg = pipe(
    createTransactionMessage({ version: 1 }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(value, m),
    (m) => appendTransactionMessageInstructions(ixs, m),
    (m) => setTransactionMessageComputeUnitLimit(10_000, m),
    (m) => setTransactionMessageLoadedAccountsDataSizeLimit(100_000, m),
  );
  const tx = await signTransactionMessageWithSigners(msg);
  await r.sendTransaction!(getBase64EncodedWireTransaction(tx), { encoding: 'base64' }).send();
}

describe('send with TransactionModifyingSigner (wallet-standard)', () => {
  /** A transfer from `payer` whose account metas carry no signer object (the fee payer signer covers it). */
  const bare = (payer: KeyPairSigner, to: Address): Instruction[] => [
    { programAddress: SYSTEM_PROGRAM, accounts: [{ address: payer.address, role: AccountRole.WRITABLE_SIGNER }, { address: to, role: AccountRole.WRITABLE }], data: concat(u32(2), u64(1n)) },
  ];
  const otherBlockhash = (m: ReturnType<typeof decodeWire>['message']) =>
    setTransactionMessageLifetimeUsingBlockhash({ blockhash: BLOCKHASHES[2]!, lastValidBlockHeight: 10_000n }, m as never);

  it('a modifying fee payer that leaves the message unchanged is signed, sent once and confirmed', async () => {
    const r = sendRpc({ status: () => ({ err: null }) });
    const { payer, ixs } = await basic();
    const wallet = modifyingSigner(payer);
    const ix: Instruction = { ...ixs[0]!, accounts: [{ ...ixs[0]!.accounts![0]!, signer: wallet } as AccountSignerMeta, ixs[0]!.accounts![1]!] };
    const out = await sdk.send(r.rpc, { instructions: [ix], feePayer: wallet, signers: [] });
    assert.equal(out.status, 'confirmed', show(out));
    assertSentOnce(r);
    assert.equal(out.signature, decodeWire(r.sent[0]!).signature);
  });

  it('a modifying signer in `signers` that leaves the message unchanged works too', async () => {
    const r = sendRpc({ status: () => ({ err: null }) });
    const payer = await generateKeyPairSigner();
    const other = await generateKeyPairSigner();
    const wallet = modifyingSigner(other);
    const ix: Instruction = { programAddress: SYSTEM_PROGRAM, accounts: [{ address: other.address, role: AccountRole.WRITABLE_SIGNER }, { address: payer.address, role: AccountRole.WRITABLE }], data: concat(u32(2), u64(1n)) };
    const out = await sdk.send(r.rpc, { instructions: [ix], feePayer: payer, signers: [wallet] });
    assert.equal(out.status, 'confirmed', show(out));
    assertSentOnce(r);
  });

  for (const what of ['changes the blockhash', 'adds an instruction'] as const) {
    it(`a modifying signer that ${what} is refused with SIGNER_MODIFIED_TRANSACTION and nothing is sent`, async () => {
      const r = sendRpc({ status: () => ({ err: null }) });
      const payer = await generateKeyPairSigner();
      const ixs = bare(payer, (await generateKeyPairSigner()).address);
      const extra = transferIx(payer, (await generateKeyPairSigner()).address, 999n);
      const edit = what === 'changes the blockhash' ? otherBlockhash : (m: ReturnType<typeof decodeWire>['message']) => appendTransactionMessageInstructions([extra], m as never);
      const wallet = modifyingSigner(payer, edit);
      await rejectsWith(() => sdk.send(r.rpc, { instructions: ixs, feePayer: wallet, signers: [] }), 'SIGNER_MODIFIED_TRANSACTION', what);
      assert.equal(r.count('sendTransaction'), 0, 'fail closed: nothing sent');
    });
  }

  it('safeguard removed: without the comparison the modified transaction is sent', async () => {
    const r = sendRpc({ status: () => ({ err: null }) });
    const payer = await generateKeyPairSigner();
    await uncheckedSend(r.rpc, modifyingSigner(payer, otherBlockhash), bare(payer, (await generateKeyPairSigner()).address));
    assert.equal(r.count('sendTransaction'), 1);
    assert.notEqual(decodeWire(r.sent[0]!).message.lifetimeConstraint, undefined);
    assert.equal(show(decodeWire(r.sent[0]!).message.lifetimeConstraint).includes(BLOCKHASHES[2]!), true, 'the signer\'s blockhash went out');
  });
});

// ---------- "never rebuilds": a ledger where every received transaction lands late ----------
/** Every transaction sent lands; its status only becomes visible after `hiddenPolls` status polls. */
function lateLedger(hiddenPolls: number) {
  const r = sendRpc({ heightStep: 40n, status: (n) => (n >= hiddenPolls ? { err: null } : undefined) });
  /** Distinct transactions that landed with the same effect (same instruction data and accounts). */
  const landedEffects = () => {
    const seen = new Map<string, number>();
    for (const b of r.sent) {
      const { message } = decodeWire(b);
      const ixs: readonly Instruction[] = message.instructions;
      const key = show(ixs.map((i) => [i.programAddress, i.accounts?.map((a) => a.address), i.data]));
      seen.set(key, (seen.get(key) ?? 0) + 1);
    }
    return [...seen.values()];
  };
  return { ...r, landedEffects };
}
const assertNoDuplicateEffect = (ledger: ReturnType<typeof lateLedger>) =>
  assert.deepEqual(ledger.landedEffects(), [1], 'each intended effect landed at most once');

/** Safeguard removed: a sender that rebuilds with a fresh blockhash when the outcome is unknown. */
async function rebuildingSend(rpc: ReturnType<typeof lateLedger>['rpc'], payer: KeyPairSigner, ixs: Instruction[]) {
  const r = rpc as unknown as Record<string, (...a: unknown[]) => { send(): Promise<never> }>;
  for (let attempt = 0; attempt < 2; attempt++) {
    const { value } = (await r.getLatestBlockhash!().send()) as { value: { blockhash: never; lastValidBlockHeight: bigint } };
    const msg = pipe(
      createTransactionMessage({ version: 1 }),
      (m) => setTransactionMessageFeePayerSigner(payer, m),
      (m) => setTransactionMessageLifetimeUsingBlockhash(value, m),
      (m) => appendTransactionMessageInstructions(ixs, m),
      (m) => setTransactionMessageComputeUnitLimit(10_000, m),
      (m) => setTransactionMessageLoadedAccountsDataSizeLimit(100_000, m),
    );
    const tx = await signTransactionMessageWithSigners(msg);
    await r.sendTransaction!(getBase64EncodedWireTransaction(tx), { encoding: 'base64' }).send();
    const st = (await r.getSignatureStatuses!([getSignatureFromTransaction(tx)]).send()) as { value: unknown[] };
    if (st.value[0] !== null) return;
  }
}

describe('send never re-sends or rebuilds (with safeguard-removed counterpart)', () => {
  it('SDK send: one landing even when confirmation is slow', async () => {
    const ledger = lateLedger(1_000);
    const { payer, ixs } = await basic();
    const out = await sdk.send(ledger.rpc, { instructions: ixs, feePayer: payer, signers: [payer] });
    assert.equal(out.status, 'unknown');
    assertNoDuplicateEffect(ledger);
  });
  it('safeguard removed: a rebuilding sender produces a duplicate, and the check catches it', async () => {
    const ledger = lateLedger(1_000);
    const { payer, ixs } = await basic();
    await rebuildingSend(ledger.rpc, payer, ixs);
    assert.throws(() => assertNoDuplicateEffect(ledger), /at most once/);
  });
});

// ---------- checkOutcome ----------
type Checker = (rpc: unknown, sig: Signature, lvbh: bigint) => Promise<CheckedOutcome>;
/** RPC for checkOutcome: the signature is found iff `found()`; block height is `height()`. */
/** `finalizedSlot` is the finalized slot reported with the height; `statusSlot` the status node's context slot. */
function outcomeRpc(p: { found: () => { err: unknown } | undefined; height: () => bigint; finalizedSlot?: bigint; statusSlot?: bigint }) {
  return makeRpc({
    getBlockHeight: () => p.height(),
    getEpochInfo: () => ({ absoluteSlot: p.finalizedSlot ?? 5n, blockHeight: p.height(), epoch: 0n, slotIndex: 5n, slotsInEpoch: 432_000n, transactionCount: null }),
    getSlot: () => 5n,
    getSignatureStatuses: (sigs: string[]) => ({
      context: { slot: p.statusSlot ?? 6n },
      value: sigs.map(() => {
        const f = p.found();
        return f === undefined ? null : { slot: 9n, confirmations: null, err: f.err, confirmationStatus: 'finalized', status: f.err ? { Err: f.err } : { Ok: null } };
      }),
    }),
    getTransaction: (_sig: string, c: unknown) => {
      const f = p.found();
      void c;
      return f === undefined ? null : { slot: 9n, blockTime: null, version: 1, meta: { err: f.err, logMessages: [], fee: 5000n, preBalances: [], postBalances: [] }, transaction: { signatures: [_sig], message: { accountKeys: [], instructions: [], recentBlockhash: '' } } };
    },
  } as never);
}
const SIG = '5VERv8NMvzbJMEkV8xnrLkEaWRtSz9CosKDYjCJjBRnbJLgp8uirBgmQpjKhoR4tjF3ZpRzrFmBV6UjKdiSZkQUW' as Signature;

/** The checkOutcome contract as scenarios; each throws if `check` violates it. */
const scenarios: Record<string, (check: Checker) => Promise<void>> = {
  'found ok -> confirmed': async (check) => {
    const out = await check(outcomeRpc({ found: () => ({ err: null }), height: () => 100n }).rpc, SIG, 250n);
    assert.equal(out.status, 'confirmed', show(out));
    if (out.status === 'confirmed') assert.equal(out.signature, SIG);
  },
  'found with error -> failed': async (check) => {
    const out = await check(outcomeRpc({ found: () => ({ err: { InstructionError: [0, { Custom: 6001 }] } }), height: () => 300n }).rpc, SIG, 250n);
    assert.equal(out.status, 'failed', show(out));
  },
  'absent, block height below lastValidBlockHeight -> not expired': async (check) => {
    const out = await check(outcomeRpc({ found: () => undefined, height: () => 249n }).rpc, SIG, 250n);
    assert.notEqual(out.status, 'expired', show(out));
    assert.equal(out.status, 'unknown');
  },
  'absent, block height equal to lastValidBlockHeight -> not expired (it has not passed yet)': async (check) => {
    const out = await check(outcomeRpc({ found: () => undefined, height: () => 250n }).rpc, SIG, 250n);
    assert.notEqual(out.status, 'expired', show(out));
  },
  'absent, block height passed lastValidBlockHeight -> expired': async (check) => {
    const out = await check(outcomeRpc({ found: () => undefined, height: () => 251n }).rpc, SIG, 250n);
    assert.equal(out.status, 'expired', show(out));
  },
  'absent past lastValidBlockHeight, status node caught up to the finalized slot -> expired': async (check) => {
    const out = await check(outcomeRpc({ found: () => undefined, height: () => 400n, finalizedSlot: 1_000n, statusSlot: 1_000n }).rpc, SIG, 250n);
    assert.equal(out.status, 'expired', show(out));
  },
  'absent past lastValidBlockHeight, but the status node lags the finalized slot -> unknown, never expired': async (check) => {
    const out = await check(outcomeRpc({ found: () => undefined, height: () => 400n, finalizedSlot: 1_000n, statusSlot: 999n }).rpc, SIG, 250n);
    assert.equal(out.status, 'unknown', show(out));
  },
  'finalized height and slot come from one finalized read (getEpochInfo, commitment finalized)': async (check) => {
    const r = outcomeRpc({ found: () => undefined, height: () => 400n, finalizedSlot: 1_000n, statusSlot: 1_000n });
    await check(r.rpc, SIG, 250n);
    const reads = r.calls.filter((c) => c.method === 'getEpochInfo');
    assert.ok(reads.length >= 1 && reads.every((c) => cfg(c.args[0]).commitment === 'finalized'), show(r.calls.map((c) => [c.method, c.args[0]])));
    const statuses = r.calls.filter((c) => c.method === 'getSignatureStatuses');
    assert.ok(statuses.length >= 1 && statuses.every((c) => cfg(c.args[1]).searchTransactionHistory === true), 'searches transaction history');
    assert.ok(r.calls.findIndex((c) => c.method === 'getEpochInfo') < r.calls.findIndex((c) => c.method === 'getSignatureStatuses'), 'height read before statuses');
  },
  'lands while the height is read: never expired ("still not found" is re-checked after the height)': async (check) => {
    let heightRead = false;
    const out = await check(
      outcomeRpc({ found: () => (heightRead ? { err: null } : undefined), height: () => ((heightRead = true), 251n) }).rpc,
      SIG,
      250n,
    );
    assert.notEqual(out.status, 'expired', show(out));
  },
};

describe('checkOutcome', () => {
  for (const [name, run] of Object.entries(scenarios)) it(name, () => run((rpc, sig, l) => sdk.checkOutcome(rpc, sig, l)));

  it('every getTransaction call passes maxSupportedTransactionVersion: 1', async () => {
    for (const found of [() => ({ err: null }), () => ({ err: { InstructionError: [0, { Custom: 1 }] } }), () => undefined]) {
      const r = outcomeRpc({ found, height: () => 300n });
      await sdk.checkOutcome(r.rpc, SIG, 250n);
      for (const c of r.calls.filter((x) => x.method === 'getTransaction')) assert.equal(cfg(c.args[1]).maxSupportedTransactionVersion, 1);
    }
  });

  it('safeguard removed: "absent means expired" violates the contract and the scenarios catch it', async () => {
    const naive: Checker = async (rpc, sig) => {
      const r = rpc as Record<string, (...a: unknown[]) => { send(): Promise<{ value: unknown[] }> }>;
      const st = (await r.getSignatureStatuses!([sig]).send()).value[0];
      return st === null ? { status: 'expired' } : { status: 'confirmed', signature: sig, slot: 9n };
    };
    await assert.rejects(() => scenarios['absent, block height below lastValidBlockHeight -> not expired']!(naive));
    await assert.rejects(() => scenarios['lands while the height is read: never expired ("still not found" is re-checked after the height)']!(naive));
  });

  it('safeguard removed: "height passed and absent means expired" (no slot check) is caught by the lagging-node scenario', async () => {
    const heightOnly: Checker = async (rpc, sig, lvbh) => {
      const r = rpc as Record<string, (...a: unknown[]) => { send(): Promise<never> }>;
      const { blockHeight } = (await r.getEpochInfo!({ commitment: 'finalized' }).send()) as { blockHeight: bigint };
      const st = ((await r.getSignatureStatuses!([sig], { searchTransactionHistory: true }).send()) as { value: unknown[] }).value[0];
      if (st !== null) return { status: 'confirmed', signature: sig, slot: 9n };
      return blockHeight > lvbh ? { status: 'expired' } : { status: 'unknown', signature: sig, lastValidBlockHeight: lvbh };
    };
    await scenarios['absent past lastValidBlockHeight, status node caught up to the finalized slot -> expired']!(heightOnly);
    await assert.rejects(() => scenarios['absent past lastValidBlockHeight, but the status node lags the finalized slot -> unknown, never expired']!(heightOnly));
  });

  it('history not available: a node that does not search history answers like a lagging node (unknown), and the README says so', async () => {
    // A node without transaction history reports a context slot from before the finalized slot it cannot serve.
    const r = outcomeRpc({ found: () => undefined, height: () => 10_000n, finalizedSlot: 50_000n, statusSlot: 49_000n });
    assert.equal((await sdk.checkOutcome(r.rpc, SIG, 250n)).status, 'unknown');
    const readme = (await import('node:fs')).readFileSync(new URL('../README.md', import.meta.url), 'utf8');
    assert.match(readme, /transaction history/, 'README documents the history requirement');
  });
});
