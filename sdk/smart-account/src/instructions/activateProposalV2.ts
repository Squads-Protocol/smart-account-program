import { PublicKey } from "@solana/web3.js";
import { getProposalPda } from "../pda";
import { createActivateProposalV2Instruction, PROGRAM_ID } from "../generated";
import { patchInstructionEvd } from "../utils";

export function activateProposalV2({
  settingsPda,
  transactionIndex,
  signer,
  extraVerificationData,
  isExternalSigner,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  extraVerificationData?: Uint8Array | null;
  isExternalSigner?: boolean;
  programId?: PublicKey;
}) {
  const [proposalPda] = getProposalPda({
    settingsPda,
    transactionIndex,
    programId,
  });

  const ix = createActivateProposalV2Instruction(
    {
      settings: settingsPda,
      proposal: proposalPda,
      signer,
    },
    { extraVerificationData: null },
    programId
  );
  if (extraVerificationData && extraVerificationData.length > 0) {
    patchInstructionEvd(ix, extraVerificationData);
  } else if (!isExternalSigner) {
    const signerMeta = ix.keys.find((k) => k.pubkey.equals(signer));
    if (signerMeta) signerMeta.isSigner = true;
  }
  return ix;
}
