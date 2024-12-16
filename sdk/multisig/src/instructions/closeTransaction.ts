import { PublicKey } from "@solana/web3.js";
import {
  createCloseTransactionInstruction,
  PROGRAM_ID,
} from "../generated";
import { getProposalPda, getTransactionPda } from "../pda";

export function closeTransaction({
  settingsPda,
  rentCollector,
  transactionIndex,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  rentCollector: PublicKey;
  transactionIndex: bigint;
  programId?: PublicKey;
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
      settings: settingsPda,
      rentCollector,
      proposal: proposalPda,
      transaction: transactionPda,
    },
    programId
  );
}
