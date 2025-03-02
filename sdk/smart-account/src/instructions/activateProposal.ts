import { PublicKey } from "@solana/web3.js";
import { getProposalPda } from "../pda";
import { createActivateProposalInstruction, PROGRAM_ID } from "../generated";

export function activateProposal({
  settingsPda,
  transactionIndex,
  signer,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  programId?: PublicKey;
}) {
  const [proposalPda] = getProposalPda({
    settingsPda,
    transactionIndex,
    programId,
  });

  return createActivateProposalInstruction(
    {
      settings: settingsPda,
      proposal: proposalPda,
      signer,
    },
    programId
  );
}
