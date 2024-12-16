import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import { translateAndThrowAnchorError } from "../errors.js";
import * as transactions from "../transactions/index.js";

/**
 * Closes a VaultBatchTransaction belonging to the Batch and Proposal defined by `batchIndex`.
 * VaultBatchTransaction can be closed if either:
 * - it's marked as executed within the batch;
 * - the proposal is in a terminal state: `Executed`, `Rejected`, or `Cancelled`.
 * - the proposal is stale and not `Approved`.
 */
export async function closeBatchTransaction({
  connection,
  feePayer,
  settingsPda,
  rentCollector,
  batchIndex,
  transactionIndex,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  rentCollector: PublicKey;
  batchIndex: bigint;
  transactionIndex: number;
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.closeBatchTransaction({
    blockhash,
    feePayer: feePayer.publicKey,
    rentCollector,
    batchIndex,
    transactionIndex,
    settingsPda,
    programId,
  });

  tx.sign([feePayer]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
