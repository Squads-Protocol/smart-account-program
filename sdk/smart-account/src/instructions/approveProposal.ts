import { getProposalPda } from "../pda";
import { createApproveProposalInstruction } from "../generated";
import { PublicKey } from "@solana/web3.js";

export function approveProposal({
  settingsPda,
  transactionIndex,
  signer,
  memo,
  programId,
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

  return createApproveProposalInstruction(
    { settings: settingsPda, proposal: proposalPda, signer },
    { args: { memo: memo ?? null } },
    programId
  );
}
