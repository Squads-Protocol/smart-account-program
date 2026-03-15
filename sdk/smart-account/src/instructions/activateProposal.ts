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

  const ix = createActivateProposalInstruction(
    {
      settings: settingsPda,
      proposal: proposalPda,
      signer,
    },
    programId
  );
  const signerMeta = ix.keys.find((k) => k.pubkey.equals(signer));
  if (signerMeta) signerMeta.isSigner = true;
  return ix;
}
