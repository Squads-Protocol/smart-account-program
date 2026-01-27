import { PublicKey } from "@solana/web3.js";
import {
  createCloseTransactionInstruction,
  PROGRAM_ID,
} from "../generated";
import { getProposalPda, getTransactionPda } from "../pda";

export function closeTransaction({
  settingsPda,
  transactionRentCollector,
  transactionIndex,
  programId = PROGRAM_ID,
  proposalRentCollector = transactionRentCollector,
}: {
  settingsPda: PublicKey;
  transactionRentCollector: PublicKey;
  transactionIndex: bigint;
  programId?: PublicKey;
  proposalRentCollector?: PublicKey;
}) {
  const [proposalPda] = getProposalPda({
    settingsPda,
    transactionIndex,
    programId,
  });
  const [transactionPda] = getTransactionPda({
    settingsPda,
    transactionIndex: transactionIndex,
    programId,
  });

  return createCloseTransactionInstruction(
    {
      consensusAccount: settingsPda,
      proposal: proposalPda,
      proposalRentCollector,
      transaction: transactionPda,
      transactionRentCollector,
      program: programId,
    },
    programId
  );
}
