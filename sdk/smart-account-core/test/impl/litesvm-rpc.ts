// A minimal kit-Rpc look-alike backed by LiteSVM, covering exactly the RPC methods src/ calls.
// It lets the impl checks drive fetch*/send/checkOutcome/resolveCreatedAccount end to end on the
// pinned mainnet binary. Test-only; not part of the package.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import {
  type Address,
  address,
  getAddressDecoder,
  getBase64Decoder,
  getCompiledTransactionMessageDecoder,
  getSignatureFromTransaction,
  getTransactionDecoder,
  lamports,
  type Rpc,
  type Signature,
  type SolanaRpcApi,
  type Transaction,
} from '@solana/kit';
import { FailedTransactionMetadata, LiteSVM } from 'litesvm';

export const PROGRAM_ID = address('SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG');
const FIXTURES = fileURLToPath(new URL('../fixtures/mainnet/', import.meta.url));
const BPF_UPGRADEABLE = 'BPFLoaderUpgradeab1e11111111111111111111111';
const ACCOUNT_BASE_SIZE = 64;

const b64 = (b: Uint8Array) => getBase64Decoder().decode(b);
const getAddressFromBytes = (b: Uint8Array): Address => getAddressDecoder().decode(b);
const ok = <T>(value: T) => ({ send: async () => value });

type Stored = { slot: bigint; err: unknown; wire: string; pre: bigint[]; post: bigint[]; logs: string[] };

function liteErr(res: FailedTransactionMetadata): unknown {
  const e: unknown = res.err();
  if (typeof e === 'number') return `Fieldless(${e})`;
  if (e && typeof e === 'object' && 'index' in e && 'err' in e && typeof e.err === 'function') {
    const inner: unknown = e.err();
    const ie = inner && typeof inner === 'object' && 'code' in inner ? { Custom: BigInt(Number(inner.code)) } : `Instruction(${String(inner)})`;
    return { InstructionError: [Number(e.index), ie] };
  }
  return String(e);
}

export function createLiteSvmRpc() {
  const svm = new LiteSVM();
  svm.addProgram(PROGRAM_ID, readFileSync(`${FIXTURES}smart_account_program.so`));
  const pc = JSON.parse(readFileSync(`${FIXTURES}program_config.json`, 'utf8'));
  const pcData = new Uint8Array(Buffer.from(pc.dataBase64, 'base64'));
  svm.setAccount({
    address: address(pc.address),
    data: pcData,
    executable: false,
    lamports: lamports(BigInt(pc.lamports)),
    programAddress: PROGRAM_ID,
    space: BigInt(pcData.length),
  });
  svm.airdrop(address(pc.decoded.treasury), lamports(1_000_000_000n));

  const state = { slot: 100n, blockHeight: 100n, lastValidBlockHeight: 250n, dropSends: false };
  const txs = new Map<Signature, Stored>();

  const decode = (wire: string): Transaction => getTransactionDecoder().decode(Buffer.from(wire, 'base64'));
  const keysOf = (tx: Transaction) => getCompiledTransactionMessageDecoder().decode(tx.messageBytes).staticAccounts;
  const balances = (keys: readonly Address[]) => keys.map((k) => svm.getBalance(k) ?? 0n);
  const loadedSize = (keys: readonly Address[]) =>
    keys.reduce((n, k) => {
      const acc = svm.getAccount(k);
      if (!acc.exists) return n + ACCOUNT_BASE_SIZE;
      let size = acc.data.length + ACCOUNT_BASE_SIZE;
      if (acc.executable && acc.programAddress === BPF_UPGRADEABLE && acc.data.length === 36) {
        const programData = svm.getAccount(getAddressFromBytes(acc.data.subarray(4)));
        if (programData.exists) size += programData.data.length + ACCOUNT_BASE_SIZE;
      }
      return n + size;
    }, 0);
  const accountInfo = (a: Address) => {
    const acc = svm.getAccount(a);
    if (!acc.exists) return null;
    return {
      owner: acc.programAddress,
      lamports: acc.lamports,
      executable: acc.executable,
      space: BigInt(acc.data.length),
      rentEpoch: 0n,
      data: [b64(acc.data), 'base64'] as const,
    };
  };
  const ctx = () => ({ slot: state.slot });

  const api = {
    getLatestBlockhash: () =>
      ok({ context: ctx(), value: { blockhash: svm.latestBlockhash(), lastValidBlockHeight: state.lastValidBlockHeight } }),
    getBlockHeight: () => ok(state.blockHeight),
    getEpochInfo: () => ok({ absoluteSlot: state.slot, blockHeight: state.blockHeight, epoch: 0n, slotIndex: state.slot, slotsInEpoch: 432_000n, transactionCount: null }),
    getAccountInfo: (a: Address) => ok({ context: ctx(), value: accountInfo(a) }),
    getMultipleAccounts: (as: Address[]) => ok({ context: ctx(), value: as.map(accountInfo) }),
    getBalance: (a: Address) => ok({ context: ctx(), value: svm.getBalance(a) ?? 0n }),
    getTokenAccountsByOwner: (owner: Address, filter: { programId: Address }) =>
      ok({
        context: ctx(),
        value: svm
          .getProgramAccounts(filter.programId)
          .filter((acc) => acc.data.length >= 165 && getAddressFromBytes(acc.data.subarray(32, 64)) === owner)
          .map((acc) => ({ pubkey: acc.address, account: accountInfo(acc.address) })),
      }),
    getProgramAccounts: (program: Address, config: { filters?: { memcmp: { offset: bigint; bytes: string } }[] }) =>
      ok(
        svm
          .getProgramAccounts(program)
          .filter((acc) =>
            (config.filters ?? []).every((f) => {
              const want = Buffer.from(f.memcmp.bytes, 'base64');
              const o = Number(f.memcmp.offset);
              return Buffer.from(acc.data.subarray(o, o + want.length)).equals(want);
            }),
          )
          .map((acc) => ({ pubkey: acc.address, account: accountInfo(acc.address) })),
      ),
    simulateTransaction: (wire: string) => {
      const tx = decode(wire);
      svm.withSigverify(false);
      const res = svm.simulateTransaction(tx);
      svm.withSigverify(true);
      const keys = keysOf(tx);
      if (res instanceof FailedTransactionMetadata) {
        return ok({ context: ctx(), value: { err: liteErr(res), logs: res.meta().logs(), unitsConsumed: res.meta().computeUnitsConsumed(), loadedAccountsDataSize: loadedSize(keys) } });
      }
      return ok({ context: ctx(), value: { err: null, logs: res.meta().logs(), unitsConsumed: res.meta().computeUnitsConsumed(), loadedAccountsDataSize: loadedSize(keys) } });
    },
    sendTransaction: (wire: string) => {
      const tx = decode(wire);
      const signature = getSignatureFromTransaction(tx);
      if (state.dropSends) return ok(signature);
      const keys = keysOf(tx);
      const pre = balances(keys);
      const res = svm.sendTransaction(tx);
      state.slot += 1n;
      const failed = res instanceof FailedTransactionMetadata;
      txs.set(signature, {
        slot: state.slot,
        err: failed ? liteErr(res) : null,
        wire,
        pre,
        post: balances(keys),
        logs: failed ? res.meta().logs() : res.logs(),
      });
      svm.expireBlockhash();
      return ok(signature);
    },
    getSignatureStatuses: (sigs: Signature[]) =>
      ok({
        context: ctx(),
        value: sigs.map((s) => {
          const t = txs.get(s);
          return t ? { slot: t.slot, confirmations: null, err: t.err, confirmationStatus: 'confirmed', status: t.err ? { Err: t.err } : { Ok: null } } : null;
        }),
      }),
    getTransaction: (s: Signature) => {
      const t = txs.get(s);
      return ok(
        t
          ? {
              slot: t.slot,
              blockTime: null,
              version: 1,
              transaction: [t.wire, 'base64'],
              meta: { err: t.err, fee: 5000n, preBalances: t.pre, postBalances: t.post, logMessages: t.logs, loadedAddresses: { writable: [], readonly: [] } },
            }
          : null,
      );
    },
  };
  return { svm, state, txs, rpc: api as unknown as Rpc<SolanaRpcApi> };
}
