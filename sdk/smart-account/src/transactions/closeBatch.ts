import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions/index.js";

/**
 * Closes Batch and the corresponding Proposal accounts for proposals in terminal states:
 * `Executed`, `Rejected`, or `Cancelled` or stale proposals that aren't Approved.
 *
 * This instruction is only allowed to be executed when all `VaultBatchTransaction` accounts
 * in the `batch` are already closed: `batch.size == 0`.
 */
export function closeBatch({
  blockhash,
  feePayer,
  settingsPda,
  batchRentCollector,
  batchIndex,
  programId,
  proposalRentCollector = batchRentCollector,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  batchRentCollector: PublicKey;
  batchIndex: bigint;
  proposalRentCollector?: PublicKey;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.closeBatch({
        settingsPda,
        batchRentCollector,
        batchIndex,
        programId,
        proposalRentCollector
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
