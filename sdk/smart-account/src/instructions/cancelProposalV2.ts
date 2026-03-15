import { PublicKey, SystemProgram } from "@solana/web3.js";
import { createCancelProposalV2Instruction, PROGRAM_ID } from "../generated";
import { getProposalPda } from "../pda";
import { patchInstructionEvd } from "../utils";

export function cancelProposalV2({
  settingsPda,
  transactionIndex,
  signer,
  memo,
  extraVerificationData,
  isExternalSigner,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  isExternalSigner?: boolean;
  programId?: PublicKey;
}) {
  const [proposalPda] = getProposalPda({
    settingsPda,
    transactionIndex,
    programId,
  });

  const ix = createCancelProposalV2Instruction(
    { consensusAccount: settingsPda, proposal: proposalPda, signer, systemProgram: SystemProgram.programId, program: programId },
    { args: { memo: memo ?? null }, extraVerificationData: null },
    programId
  );
  const consensusMeta = ix.keys.find((k) => k.pubkey.equals(settingsPda));
  if (consensusMeta) consensusMeta.isWritable = true;
  if (extraVerificationData && extraVerificationData.length > 0) {
    patchInstructionEvd(ix, extraVerificationData);
  } else if (!isExternalSigner) {
    const signerMeta = ix.keys.find((k) => k.pubkey.equals(signer));
    if (signerMeta) signerMeta.isSigner = true;
  }
  return ix;
}
