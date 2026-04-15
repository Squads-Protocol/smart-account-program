import { PublicKey } from "@solana/web3.js";
import { createCreateProposalV2Instruction, PROGRAM_ID } from "../generated";
import { getProposalPda } from "../pda";
import { patchInstructionEvd } from "../utils";

export function createProposalV2({
  settingsPda,
  creator,
  rentPayer,
  transactionIndex,
  isDraft = false,
  extraVerificationData,
  isExternalSigner,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  creator: PublicKey;
  rentPayer?: PublicKey;
  transactionIndex: bigint;
  isDraft?: boolean;
  extraVerificationData?: Uint8Array | null;
  isExternalSigner?: boolean;
  programId?: PublicKey;
}) {
  const [proposalPda] = getProposalPda({
    settingsPda,
    transactionIndex,
    programId,
  });

  if (transactionIndex > Number.MAX_SAFE_INTEGER) {
    throw new Error("transactionIndex is too large");
  }

  const ix = createCreateProposalV2Instruction(
    {
      creator,
      rentPayer: rentPayer ?? creator,
      consensusAccount: settingsPda,
      proposal: proposalPda,
      program: programId,
    },
    { args: { transactionIndex: Number(transactionIndex), draft: isDraft }, extraVerificationData: null },
    programId
  );
  if (extraVerificationData && extraVerificationData.length > 0) {
    patchInstructionEvd(ix, extraVerificationData);
  } else if (!isExternalSigner) {
    const creatorMeta = ix.keys.find((k) => k.pubkey.equals(creator));
    if (creatorMeta) creatorMeta.isSigner = true;
  }
  return ix;
}
