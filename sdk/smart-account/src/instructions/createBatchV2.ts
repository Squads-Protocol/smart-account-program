import { PublicKey } from "@solana/web3.js";
import { createCreateBatchV2Instruction, PROGRAM_ID } from "../generated";
import { getTransactionPda } from "../pda";
import { patchInstructionEvd } from "../utils";

export function createBatchV2({
  settingsPda,
  creator,
  rentPayer,
  batchIndex,
  accountIndex,
  memo,
  extraVerificationData,
  isExternalSigner,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  creator: PublicKey;
  rentPayer?: PublicKey;
  batchIndex: bigint;
  accountIndex: number;
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  isExternalSigner?: boolean;
  programId?: PublicKey;
}) {
  const [batchPda] = getTransactionPda({
    settingsPda,
    transactionIndex: batchIndex,
    programId,
  });

  const ix = createCreateBatchV2Instruction(
    {
      settings: settingsPda,
      creator,
      rentPayer: rentPayer ?? creator,
      batch: batchPda,
    },
    { args: { accountIndex, memo: memo ?? null }, extraVerificationData: null },
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
