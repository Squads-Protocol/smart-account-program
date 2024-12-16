import { PublicKey } from "@solana/web3.js";
import { createCancelProposalInstruction, PROGRAM_ID } from "../generated";
import { getProposalPda } from "../pda";

export function cancelProposal({
  settingsPda,
  transactionIndex,
  signer,
  memo,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  memo?: string;
  programId?: PublicKey;
}) {
  const [proposalPda] = getProposalPda({
    settingsPda,
    transactionIndex,
    programId,
  });

  return createCancelProposalInstruction(
    { settings: settingsPda, proposal: proposalPda, signer },
    { args: { memo: memo ?? null } },
    programId
  );
}
