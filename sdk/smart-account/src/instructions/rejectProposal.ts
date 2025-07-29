import { getProposalPda } from "../pda";
import { createRejectProposalInstruction, PROGRAM_ID } from "../generated";
import { PublicKey } from "@solana/web3.js";

export function rejectProposal({
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

  return createRejectProposalInstruction(
    {
      consensusAccount: settingsPda,
      proposal: proposalPda,
      signer,
      program: programId,
    },
    { args: { memo: memo ?? null } },
    programId
  );
}
