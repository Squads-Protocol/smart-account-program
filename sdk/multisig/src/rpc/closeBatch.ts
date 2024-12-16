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
 * Closes Batch and the corresponding Proposal accounts for proposals in terminal states:
 * `Executed`, `Rejected`, or `Cancelled` or stale proposals that aren't Approved.
 *
 * This instruction is only allowed to be executed when all `VaultBatchTransaction` accounts
 * in the `batch` are already closed: `batch.size == 0`.
 */
export async function closeBatch({
  connection,
  feePayer,
  settingsPda,
  rentCollector,
  batchIndex,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  rentCollector: PublicKey;
  batchIndex: bigint;
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.closeBatch({
    blockhash,
    feePayer: feePayer.publicKey,
    rentCollector,
    batchIndex,
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
