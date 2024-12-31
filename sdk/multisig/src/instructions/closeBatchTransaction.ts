import { PublicKey } from "@solana/web3.js";
import {
  createCloseBatchTransactionInstruction,
  PROGRAM_ID,
} from "../generated";
import {
  getBatchTransactionPda,
  getProposalPda,
  getTransactionPda,
} from "../pda";

/**
 * Closes a VaultBatchTransaction belonging to the Batch and Proposal defined by `batchIndex`.
 * VaultBatchTransaction can be closed if either:
 * - it's marked as executed within the batch;
 * - the proposal is in a terminal state: `Executed`, `Rejected`, or `Cancelled`.
 * - the proposal is stale and not `Approved`.
 */
export function closeBatchTransaction({
  settingsPda,
  transactionRentCollector,
  batchIndex,
  transactionIndex,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  transactionRentCollector: PublicKey;
  batchIndex: bigint;
  transactionIndex: number;
  programId?: PublicKey;
}) {
  const [proposalPda] = getProposalPda({
    settingsPda,
    transactionIndex: batchIndex,
    programId,
  });
  const [batchPda] = getTransactionPda({
    settingsPda,
    transactionIndex: batchIndex,
    programId,
  });
  const [batchTransactionPda] = getBatchTransactionPda({
    settingsPda,
    batchIndex,
    transactionIndex,
    programId,
  });

  return createCloseBatchTransactionInstruction(
    {
      settings: settingsPda,
      transactionRentCollector,
      proposal: proposalPda,
      batch: batchPda,
      transaction: batchTransactionPda,
    },
    programId
  );
}
