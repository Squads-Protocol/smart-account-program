import { PublicKey } from "@solana/web3.js";
import {
  createCloseSettingsTransactionInstruction,
  PROGRAM_ID,
} from "../generated";
import { getProposalPda, getTransactionPda } from "../pda";

export function closeSettingsTransaction({
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

  return createCloseSettingsTransactionInstruction(
    {
      settings: settingsPda,
      transactionRentCollector,
      proposal: proposalPda,
      proposalRentCollector,
      transaction: transactionPda,
    },
    programId
  );
}
