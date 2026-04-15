import { PublicKey, SystemProgram } from "@solana/web3.js";
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

  const ix = createCancelProposalInstruction(
    { consensusAccount: settingsPda, proposal: proposalPda, signer, systemProgram: SystemProgram.programId, program: programId },
    { args: { memo: memo ?? null } },
    programId
  );
  const consensusMeta = ix.keys.find((k) => k.pubkey.equals(settingsPda));
  if (consensusMeta) consensusMeta.isWritable = true;
  const signerMeta = ix.keys.find((k) => k.pubkey.equals(signer));
  if (signerMeta) signerMeta.isSigner = true;
  return ix;
}
