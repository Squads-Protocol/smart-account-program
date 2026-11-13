import {
  addSignersToTransactionMessage,
  appendTransactionMessageInstructions,
  compileTransaction,
  createTransactionMessage,
  type GetBlockHeightApi,
  type GetEpochInfoApi,
  type GetLatestBlockhashApi,
  type GetSignatureStatusesApi,
  getBase64EncodedWireTransaction,
  getSignatureFromTransaction,
  getTransactionSize,
  type Instruction,
  isSolanaError,
  pipe,
  type Rpc,
  type SendTransactionApi,
  type Signature,
  type SimulateTransactionApi,
  setTransactionMessageComputeUnitLimit,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  setTransactionMessageLoadedAccountsDataSizeLimit,
  setTransactionMessagePriorityFeeLamports,
  SOLANA_ERROR__TRANSACTION__EXCEEDS_SIZE_LIMIT,
  SOLANA_ERROR__TRANSACTION__TOO_MANY_ACCOUNT_ADDRESSES,
  SOLANA_ERROR__TRANSACTION__TOO_MANY_ACCOUNTS_IN_INSTRUCTION,
  SOLANA_ERROR__TRANSACTION__TOO_MANY_INSTRUCTIONS,
  SOLANA_ERROR__TRANSACTION__TOO_MANY_SIGNER_ADDRESSES,
  signTransactionMessageWithSigners,
  type TransactionError,
  type TransactionModifyingSigner,
  type TransactionPartialSigner,
} from '@solana/kit';
import { SmartAccountError } from './errors.ts';
import type { CheckedOutcome, ProgramErrorInfo, SendOutcome } from './types.ts';

const V1_LIMITS = { bytes: 4096, instructions: 64, signatures: 12 } as const;
const LIMIT_ERRORS = [
  SOLANA_ERROR__TRANSACTION__TOO_MANY_ACCOUNT_ADDRESSES,
  SOLANA_ERROR__TRANSACTION__TOO_MANY_SIGNER_ADDRESSES,
  SOLANA_ERROR__TRANSACTION__TOO_MANY_INSTRUCTIONS,
  SOLANA_ERROR__TRANSACTION__TOO_MANY_ACCOUNTS_IN_INSTRUCTION,
  SOLANA_ERROR__TRANSACTION__EXCEEDS_SIZE_LIMIT,
] as const;
const MAX_COMPUTE_UNITS = 1_400_000;
const MAX_LOADED_DATA = 64 * 1024 * 1024;
const DATA_PAGE = 32 * 1024;
const POLL_MS = 1_000;
const MAX_CONSECUTIVE_RPC_ERRORS = 5;

export type SendRpc = Rpc<
  GetLatestBlockhashApi & SimulateTransactionApi & SendTransactionApi & GetSignatureStatusesApi & GetBlockHeightApi
>;

/** A wallet-standard (modifying) signer may not change the message: `send` refuses one that does. */
export type SendSigner = TransactionPartialSigner | TransactionModifyingSigner;

export interface SendInput {
  instructions: readonly Instruction[];
  feePayer: SendSigner;
  /** Every other signer the instructions need. */
  signers?: readonly SendSigner[];
  computeUnitLimit?: number;
  loadedAccountsDataSizeLimit?: number;
  /** Total priority fee in lamports (v1 message config). */
  priorityFeeLamports?: bigint;
}

function errorInfo(err: TransactionError, logs: readonly string[] | null): ProgramErrorInfo {
  let instructionIndex: number | null = null;
  let customCode: number | null = null;
  if (typeof err === 'object' && 'InstructionError' in err) {
    const [index, inner] = err.InstructionError;
    instructionIndex = index;
    if (typeof inner === 'object' && 'Custom' in inner) customCode = Number(inner.Custom);
  }
  return { err, instructionIndex, customCode, logs };
}

const tooLarge = (what: string, cause?: unknown) =>
  new SmartAccountError('TX_TOO_LARGE', `transaction exceeds the v1 limit: ${what}`, cause);

async function lookup(rpc: Rpc<GetSignatureStatusesApi>, signature: Signature, searchTransactionHistory: boolean) {
  const { context, value } = await rpc.getSignatureStatuses([signature], { searchTransactionHistory }).send();
  const s = value[0];
  let outcome: SendOutcome | null = null;
  if (s && (s.confirmationStatus === 'confirmed' || s.confirmationStatus === 'finalized')) {
    outcome =
      s.err === null
        ? { status: 'confirmed', signature, slot: s.slot }
        : { status: 'failed', signature, error: errorInfo(s.err, null) };
  }
  return { seen: Boolean(s), outcome, slot: context.slot };
}

/**
 * Compiles a v1 transaction, sizes its compute and loaded-data limits by simulation when not given, signs, sends,
 * and confirms against this transaction's own lastValidBlockHeight. Never rebuilds or re-signs.
 */
export async function send(rpc: SendRpc, input: SendInput): Promise<SendOutcome> {
  const { value: lifetime } = await rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
  let message = pipe(
    createTransactionMessage({ version: 1 }),
    (m) => setTransactionMessageFeePayerSigner(input.feePayer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(lifetime, m),
    (m) => appendTransactionMessageInstructions(input.instructions, m),
    (m) => addSignersToTransactionMessage([...(input.signers ?? [])], m),
    (m) => setTransactionMessagePriorityFeeLamports(input.priorityFeeLamports, m),
  );

  const compile = (m: typeof message) => {
    let tx;
    try {
      tx = compileTransaction(m);
    } catch (e) {
      if (LIMIT_ERRORS.some((code) => isSolanaError(e, code))) throw tooLarge(e instanceof Error ? e.message : 'limits', e);
      throw e;
    }
    const size = getTransactionSize(tx);
    if (size > V1_LIMITS.bytes) throw tooLarge(`${size} bytes`);
    const signatures = Object.keys(tx.signatures).length;
    if (signatures > V1_LIMITS.signatures) throw tooLarge(`${signatures} signatures`);
    return tx;
  };
  if (input.instructions.length > V1_LIMITS.instructions) throw tooLarge(`${input.instructions.length} instructions`);

  let { computeUnitLimit, loadedAccountsDataSizeLimit } = input;
  if (computeUnitLimit === undefined || loadedAccountsDataSizeLimit === undefined) {
    const probe = pipe(
      message,
      (m) => setTransactionMessageComputeUnitLimit(MAX_COMPUTE_UNITS, m),
      (m) => setTransactionMessageLoadedAccountsDataSizeLimit(MAX_LOADED_DATA, m),
    );
    const { value: sim } = await rpc
      .simulateTransaction(getBase64EncodedWireTransaction(compile(probe)), {
        encoding: 'base64',
        sigVerify: false,
        replaceRecentBlockhash: true,
        commitment: 'confirmed',
      })
      .send();
    if (sim.err !== null) {
      throw new SmartAccountError('SIMULATION_FAILED', 'simulation failed', errorInfo(sim.err, sim.logs));
    }
    if (sim.unitsConsumed === undefined || sim.loadedAccountsDataSize === undefined) {
      throw new SmartAccountError('SIMULATION_FAILED', 'the RPC did not report units consumed and loaded data size', sim);
    }
    const units = Number(sim.unitsConsumed);
    computeUnitLimit ??= Math.min(MAX_COMPUTE_UNITS, Math.ceil(units * 1.1) + 1_000);
    loadedAccountsDataSizeLimit ??= Math.min(MAX_LOADED_DATA, Math.ceil((sim.loadedAccountsDataSize + 1) / DATA_PAGE) * DATA_PAGE);
  }
  message = setTransactionMessageLoadedAccountsDataSizeLimit(
    loadedAccountsDataSizeLimit,
    setTransactionMessageComputeUnitLimit(computeUnitLimit, message),
  );

  const compiled = compile(message);
  const transaction = await signTransactionMessageWithSigners(message);
  const signed = transaction.messageBytes;
  if (signed.length !== compiled.messageBytes.length || signed.some((b, i) => b !== compiled.messageBytes[i])) {
    throw new SmartAccountError('SIGNER_MODIFIED_TRANSACTION', 'a signer changed the transaction message; nothing was sent');
  }
  const signature = getSignatureFromTransaction(transaction);
  const unknown: SendOutcome = { status: 'unknown', signature, lastValidBlockHeight: lifetime.lastValidBlockHeight };

  try {
    await rpc
      .sendTransaction(getBase64EncodedWireTransaction(transaction), { encoding: 'base64', skipPreflight: true })
      .send();
  } catch (cause) {
    return { ...unknown, cause };
  }
  let errors = 0;
  for (;;) {
    try {
      const { outcome } = await lookup(rpc, signature, false);
      if (outcome) return outcome;
      const height = await rpc.getBlockHeight({ commitment: 'confirmed' }).send();
      if (height > lifetime.lastValidBlockHeight) return (await lookup(rpc, signature, true)).outcome ?? unknown;
      errors = 0;
    } catch (cause) {
      if (++errors >= MAX_CONSECUTIVE_RPC_ERRORS) return { ...unknown, cause };
    }
    await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }
}

/**
 * Resolves an `unknown` outcome. `expired` (safe to rebuild) only when the finalized block height has passed
 * `lastValidBlockHeight`, the status node has caught up to that finalized slot, and it has no record of the signature
 * (searching transaction history). Anything less certain is `unknown`. Resolving an outcome more than ~2 minutes
 * after sending needs an RPC that serves transaction history; a node without it stays `unknown`, never `expired`.
 */
export async function checkOutcome(
  rpc: Rpc<GetSignatureStatusesApi & GetEpochInfoApi>,
  signature: Signature,
  lastValidBlockHeight: bigint,
): Promise<CheckedOutcome> {
  const { blockHeight, absoluteSlot } = await rpc.getEpochInfo({ commitment: 'finalized' }).send();
  const { seen, outcome, slot } = await lookup(rpc, signature, true);
  if (outcome) return outcome;
  if (!seen && blockHeight > lastValidBlockHeight && slot >= absoluteSlot) return { status: 'expired', signature };
  return { status: 'unknown', signature, lastValidBlockHeight };
}
