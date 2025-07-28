import { PublicKey } from "@solana/web3.js";
import {
  createCloseEmptyPolicyTransactionInstruction,
  createCloseTransactionInstruction,
  PROGRAM_ID,
} from "../generated";
import { getProposalPda, getTransactionPda } from "../pda";

export function closeEmptyPolicyTransaction({
  emptyPolicy,
  transactionRentCollector,
  transactionIndex,
  programId = PROGRAM_ID,
}: {
  emptyPolicy: PublicKey;
  transactionRentCollector: PublicKey;
  transactionIndex: bigint;
  programId?: PublicKey;
  proposalRentCollector?: PublicKey;
}) {
  const [proposalPda] = getProposalPda({
    settingsPda: emptyPolicy,
    transactionIndex,
    programId,
  });
  const [transactionPda] = getTransactionPda({
    settingsPda: emptyPolicy,
    transactionIndex: transactionIndex,
    programId,
  });

  return createCloseEmptyPolicyTransactionInstruction(
    {
      emptyPolicy,
      proposal: proposalPda,
      proposalRentCollector: transactionRentCollector,
      transaction: transactionPda,
      transactionRentCollector,
    },
    programId
  );
}
