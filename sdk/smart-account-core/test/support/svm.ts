// LiteSVM harness on the pinned mainnet binary (patterns from test/spike/litesvm-v1.test.ts), a
// direct (non-SDK) executor for test setup, and a minimal kit-shaped RPC adapter over LiteSVM with
// just the methods `send`, `checkOutcome`, `resolveCreatedAccount` and `fetch*` plausibly need.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import {
  type Address,
  type EncodedAccount,
  type Instruction,
  type KeyPairSigner,
  type ReadonlyUint8Array,
  type Transaction,
  type TransactionSigner,
  addSignersToTransactionMessage,
  address,
  appendTransactionMessageInstructions,
  createTransactionMessage,
  getAddressDecoder,
  getBase58Decoder,
  getBase64Decoder,
  getCompiledTransactionMessageDecoder,
  getSignatureFromTransaction,
  getTransactionDecoder,
  lamports,
  pipe,
  setTransactionMessageComputeUnitLimit,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  setTransactionMessageLoadedAccountsDataSizeLimit,
  signTransactionMessageWithSigners,
} from '@solana/kit';
import { FailedTransactionMetadata, LiteSVM, type TransactionMetadata } from 'litesvm';

import { PROGRAM_ID, TOKEN_2022_PROGRAM, TOKEN_PROGRAM, programConfigPda, reader } from './program.ts';
import { cfg, makeRpc, matchesFilters, wireBytes } from './rpc.ts';
import type { ProgramConfigInput } from './sdk.ts';

const FIXTURES = fileURLToPath(new URL('../fixtures/mainnet/', import.meta.url));
const SO = readFileSync(`${FIXTURES}smart_account_program.so`);
const UPGRADEABLE_LOADER = 'BPFLoaderUpgradeab1e11111111111111111111111';

export async function setupSvm() {
  const svm = new LiteSVM();
  svm.addProgram(PROGRAM_ID, SO);
  const pc = JSON.parse(readFileSync(`${FIXTURES}program_config.json`, 'utf8')) as {
    lamports: string;
    dataBase64: string;
    decoded: { smartAccountIndex: string; treasury: string; authority: string; smartAccountCreationFee: string };
  };
  const pcAddress = await programConfigPda();
  const data = new Uint8Array(Buffer.from(pc.dataBase64, 'base64'));
  svm.setAccount({
    address: pcAddress,
    data,
    executable: false,
    lamports: lamports(BigInt(pc.lamports)),
    programAddress: PROGRAM_ID,
    space: BigInt(data.length),
  });
  const treasury = address(pc.decoded.treasury);
  svm.airdrop(treasury, lamports(1_000_000_000n));
  return { svm, programConfig: readProgramConfig(svm, pcAddress) };
}

/** Decodes ProgramConfig from LiteSVM (test-side), in the shape the tests pass to buildCreateAccount. */
export function readProgramConfig(svm: LiteSVM, pcAddress: Address): ProgramConfigInput {
  const acc = svm.getAccount(pcAddress);
  if (!acc.exists) throw new Error('ProgramConfig missing');
  const r = reader(acc.data);
  r.take(8);
  const smartAccountIndex = r.u128();
  const authority = r.pubkey();
  const smartAccountCreationFee = r.u64();
  const treasury = r.pubkey();
  return { address: pcAddress, smartAccountIndex, authority, smartAccountCreationFee, treasury };
}

export function setRaw(svm: LiteSVM, addr: Address, owner: Address, data: Uint8Array, lamportsAmount?: bigint) {
  svm.setAccount({
    address: addr,
    data,
    executable: false,
    lamports: lamports(lamportsAmount ?? svm.minimumBalanceForRentExemption(BigInt(data.length))),
    programAddress: owner,
    space: BigInt(data.length),
  });
}

/** Signs and executes directly in LiteSVM (setup only; the SDK's `send` is not involved). */
export async function execute(
  svm: LiteSVM,
  payer: KeyPairSigner,
  ixs: Instruction[],
  extraSigners: readonly TransactionSigner[] = [],
): Promise<TransactionMetadata> {
  const msg = pipe(
    createTransactionMessage({ version: 1 }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: svm.latestBlockhash(), lastValidBlockHeight: 0n }, m),
    (m) => appendTransactionMessageInstructions(ixs, m),
    (m) => setTransactionMessageComputeUnitLimit(1_400_000, m),
    (m) => setTransactionMessageLoadedAccountsDataSizeLimit(16 * 1024 * 1024, m),
    (m) => addSignersToTransactionMessage([...extraSigners], m),
  );
  const res = svm.sendTransaction(await signTransactionMessageWithSigners(msg));
  svm.expireBlockhash();
  if (res instanceof FailedTransactionMetadata) {
    throw new Error(`setup tx failed: ${res.toString()}\n${res.meta().logs().join('\n')}`);
  }
  return res;
}

// ---------- RPC adapter ----------
const TX_ERRORS = readEnumNames('TransactionErrorFieldless');
const IX_ERRORS = readEnumNames('InstructionErrorFieldless');
function readEnumNames(name: string): Map<number, string> {
  const dts = readFileSync(fileURLToPath(import.meta.resolve('litesvm').replace(/index\.js$/, 'internal.d.ts')), 'utf8');
  const body = new RegExp(`enum ${name} \\{([^}]*)\\}`).exec(dts)?.[1] ?? '';
  return new Map([...body.matchAll(/(\w+)\s*=\s*(\d+)/g)].map((m) => [Number(m[2]), m[1]!]));
}
/** LiteSVM error -> the RPC JSON TransactionError shape (`{ InstructionError: [i, { Custom: n }] }`, ...). */
export function rpcError(res: FailedTransactionMetadata): unknown {
  const e = res.err();
  if (typeof e === 'number') return TX_ERRORS.get(e) ?? `TransactionError${e}`;
  if ('index' in e && 'err' in e) {
    const ie = e.err();
    const inner = typeof ie === 'number' ? (IX_ERRORS.get(ie) ?? `InstructionError${ie}`) : 'code' in ie ? { Custom: ie.code } : { BorshIoError: ie.msg };
    return { InstructionError: [e.index, inner] };
  }
  return e.toString();
}

interface TxRecord {
  slot: bigint;
  err: unknown;
  staticAccounts: Address[];
  preBalances: bigint[];
  postBalances: bigint[];
  logs: string[];
  computeUnits: bigint;
  wire: Uint8Array;
  fee: bigint;
}

/** Minimal RPC over LiteSVM. Every executed transaction lands (failed ones as failed, as if skipPreflight). */
export function createSvmRpc(svm: LiteSVM) {
  const b64dec = getBase64Decoder();
  const b58dec = getBase58Decoder();
  const addrDec = getAddressDecoder();
  const txDecoder = getTransactionDecoder();
  const msgDecoder = getCompiledTransactionMessageDecoder();
  let blockHeight = 1_000n;
  const history = new Map<string, TxRecord>();
  const slot = () => svm.getClock().slot;
  const ctx = () => ({ slot: slot() });
  const balance = (a: Address) => svm.getBalance(a) ?? 0n;

  const encodeData = (data: ReadonlyUint8Array, encoding: unknown) =>
    encoding === 'base64' || encoding === 'jsonParsed' ? [b64dec.decode(data), 'base64'] : b58dec.decode(data);
  const decimalsOf = (mint: Address) => {
    const m = svm.getAccount(mint);
    return m.exists && m.data.length >= 45 ? m.data[44]! : 0;
  };
  const encodeAccount = (acc: EncodedAccount, config: unknown) => {
    const c = cfg(config);
    let data = acc.data;
    const slice = cfg(c.dataSlice);
    if (typeof slice.offset === 'number' && typeof slice.length === 'number')
      data = data.subarray(slice.offset, slice.offset + slice.length);
    const isToken = acc.programAddress === TOKEN_PROGRAM || acc.programAddress === TOKEN_2022_PROGRAM;
    const base = { executable: acc.executable, lamports: acc.lamports, owner: acc.programAddress, space: acc.space, rentEpoch: 0n };
    if (c.encoding === 'jsonParsed' && isToken && acc.data.length >= 165) {
      const mint = addrDec.decode(acc.data.subarray(0, 32));
      const amount = reader(acc.data.subarray(64, 72)).u64();
      const decimals = decimalsOf(mint);
      return {
        ...base,
        data: {
          program: acc.programAddress === TOKEN_PROGRAM ? 'spl-token' : 'spl-token-2022',
          space: acc.space,
          parsed: {
            type: 'account',
            info: {
              mint,
              owner: addrDec.decode(acc.data.subarray(32, 64)),
              isNative: false,
              state: 'initialized',
              tokenAmount: {
                amount: amount.toString(),
                decimals,
                uiAmount: Number(amount) / 10 ** decimals,
                uiAmountString: (Number(amount) / 10 ** decimals).toString(),
              },
            },
          },
        },
      };
    }
    return { ...base, data: encodeData(data, c.encoding) };
  };
  const getAccount = (a: Address, config: unknown) => {
    const acc = svm.getAccount(a);
    return acc.exists ? encodeAccount(acc, config) : null;
  };
  const decode = (wire: unknown, config: unknown) => {
    const bytes = wireBytes(wire, config);
    const tx = txDecoder.decode(bytes) as Transaction;
    const msg = msgDecoder.decode(tx.messageBytes);
    return { tx, bytes, staticAccounts: [...msg.staticAccounts] };
  };
  /** What the runtime counts against the loaded-accounts limit: data + 64 bytes per account, programdata included. */
  const loadedSize = (accounts: Address[]) => {
    let total = 0;
    for (const a of accounts) {
      const acc = svm.getAccount(a);
      if (!acc.exists) continue;
      total += acc.data.length + 64;
      if (acc.executable && acc.programAddress === UPGRADEABLE_LOADER && acc.data.length >= 36) {
        const pd = svm.getAccount(addrDec.decode(acc.data.subarray(4, 36)));
        if (pd.exists) total += pd.data.length + 64;
      }
    }
    return total;
  };

  const handlers = {
    getLatestBlockhash: () => ({
      context: ctx(),
      value: { blockhash: svm.latestBlockhash(), lastValidBlockHeight: blockHeight + 150n },
    }),
    getBlockHeight: () => blockHeight,
    getSlot: () => slot(),
    getEpochInfo: () => ({
      absoluteSlot: slot(),
      blockHeight,
      epoch: 0n,
      slotIndex: slot(),
      slotsInEpoch: 432_000n,
      transactionCount: null,
    }),
    getBalance: (a: Address) => ({ context: ctx(), value: balance(a) }),
    getMinimumBalanceForRentExemption: (n: bigint) => svm.minimumBalanceForRentExemption(BigInt(n)),
    getAccountInfo: (a: Address, c: unknown) => ({ context: ctx(), value: getAccount(a, c) }),
    getMultipleAccounts: (as: Address[], c: unknown) => ({ context: ctx(), value: as.map((a) => getAccount(a, c)) }),
    getProgramAccounts: (program: Address, c: unknown) => {
      const list = svm
        .getProgramAccounts(program)
        .filter((acc) => matchesFilters(acc.data, cfg(c).filters))
        .map((acc) => ({ pubkey: acc.address, account: encodeAccount(acc, c) }));
      return cfg(c).withContext === true ? { context: ctx(), value: list } : list;
    },
    getTokenAccountsByOwner: (owner: Address, filter: unknown, c: unknown) => {
      const f = cfg(filter);
      const programs = f.programId !== undefined ? [String(f.programId)] : [TOKEN_PROGRAM, TOKEN_2022_PROGRAM];
      const value = programs
        .flatMap((p) => svm.getProgramAccounts(address(p)))
        .filter((acc) => acc.data.length >= 165 && addrDec.decode(acc.data.subarray(32, 64)) === owner)
        .filter((acc) => f.mint === undefined || addrDec.decode(acc.data.subarray(0, 32)) === f.mint)
        .map((acc) => ({ pubkey: acc.address, account: encodeAccount(acc, c) }));
      return { context: ctx(), value };
    },
    simulateTransaction: (wire: string, c: unknown) => {
      const { tx, staticAccounts } = decode(wire, c);
      svm.withSigverify(cfg(c).sigVerify === true).withBlockhashCheck(cfg(c).replaceRecentBlockhash !== true);
      try {
        const res = svm.simulateTransaction(tx);
        const failed = res instanceof FailedTransactionMetadata;
        const meta = res.meta();
        return {
          context: ctx(),
          value: {
            err: failed ? rpcError(res) : null,
            logs: meta.logs(),
            accounts: null,
            unitsConsumed: meta.computeUnitsConsumed(),
            loadedAccountsDataSize: loadedSize(staticAccounts),
            returnData: null,
            innerInstructions: null,
            replacementBlockhash: null,
          },
        };
      } finally {
        svm.withSigverify(true).withBlockhashCheck(true);
      }
    },
    sendTransaction: (wire: string, c: unknown) => {
      const { tx, bytes, staticAccounts } = decode(wire, c);
      const signature = getSignatureFromTransaction(tx);
      const preBalances = staticAccounts.map(balance);
      const res = svm.sendTransaction(tx);
      const failed = res instanceof FailedTransactionMetadata;
      if (failed && res.meta().logs().length === 0 && !history.has(signature)) {
        // Rejected before execution (bad signature, blockhash, duplicate...): a real RPC errors here.
        throw new Error(`test RPC: transaction rejected: ${res.toString()}`);
      }
      const meta = failed ? res.meta() : res;
      history.set(signature, {
        slot: slot(),
        err: failed ? rpcError(res) : null,
        staticAccounts,
        preBalances,
        postBalances: staticAccounts.map(balance),
        logs: meta.logs(),
        computeUnits: meta.computeUnitsConsumed(),
        wire: bytes,
        fee: preBalances[0]! - balance(staticAccounts[0]!),
      });
      svm.expireBlockhash();
      svm.warpToSlot(slot() + 1n);
      blockHeight += 1n;
      return signature;
    },
    getSignatureStatuses: (sigs: string[]) => ({
      context: ctx(),
      value: sigs.map((s) => {
        const r = history.get(s);
        return r ? { slot: r.slot, confirmations: null, err: r.err, confirmationStatus: 'finalized', status: r.err ? { Err: r.err } : { Ok: null } } : null;
      }),
    }),
    getTransaction: (sig: string, c: unknown) => {
      const r = history.get(sig);
      if (!r) return null;
      const enc = cfg(c).encoding;
      const transaction =
        enc === 'base64'
          ? [b64dec.decode(r.wire), 'base64']
          : {
              signatures: [sig],
              message: {
                accountKeys:
                  enc === 'jsonParsed'
                    ? r.staticAccounts.map((pubkey) => ({ pubkey, signer: false, writable: false, source: 'transaction' }))
                    : r.staticAccounts,
                instructions: [],
                recentBlockhash: '',
              },
            };
      return {
        slot: r.slot,
        blockTime: null,
        version: 1,
        transaction,
        meta: {
          err: r.err,
          status: r.err ? { Err: r.err } : { Ok: null },
          fee: r.fee,
          preBalances: r.preBalances,
          postBalances: r.postBalances,
          logMessages: r.logs,
          computeUnitsConsumed: r.computeUnits,
          innerInstructions: [],
          loadedAddresses: { writable: [], readonly: [] },
          preTokenBalances: [],
          postTokenBalances: [],
          rewards: [],
        },
      };
    },
  };
  const { rpc, calls, count } = makeRpc(handlers as never);
  return { rpc, calls, count, history, advanceBlocks: (n: bigint) => (blockHeight += n) };
}
