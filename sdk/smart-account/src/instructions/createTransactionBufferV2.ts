import { PublicKey } from "@solana/web3.js";
import { createCreateTransactionBufferV2Instruction, PROGRAM_ID } from "../generated";
import { patchInstructionEvd } from "../utils";

export function createTransactionBufferV2({
  settingsPda,
  transactionBuffer,
  creator,
  rentPayer,
  bufferIndex,
  accountIndex,
  finalBufferHash,
  finalBufferSize,
  buffer,
  extraVerificationData,
  isExternalSigner,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  transactionBuffer: PublicKey;
  creator: PublicKey;
  rentPayer?: PublicKey;
  bufferIndex: number;
  accountIndex: number;
  finalBufferHash: number[];
  finalBufferSize: number;
  buffer: Uint8Array;
  extraVerificationData?: Uint8Array | null;
  isExternalSigner?: boolean;
  programId?: PublicKey;
}) {
  const ix = createCreateTransactionBufferV2Instruction(
    {
      consensusAccount: settingsPda,
      transactionBuffer,
      creator,
      rentPayer: rentPayer ?? creator,
    },
    {
      args: {
        bufferIndex,
        accountIndex,
        finalBufferHash: Array.from(finalBufferHash),
        finalBufferSize,
        buffer: Buffer.from(buffer),
      },
      extraVerificationData: null,
    },
    programId
  );
  const consensusMeta = ix.keys.find((k) => k.pubkey.equals(settingsPda));
  if (consensusMeta) consensusMeta.isWritable = true;
  if (extraVerificationData && extraVerificationData.length > 0) {
    patchInstructionEvd(ix, extraVerificationData);
  } else if (!isExternalSigner) {
    const creatorMeta = ix.keys.find((k) => k.pubkey.equals(creator));
    if (creatorMeta) creatorMeta.isSigner = true;
  }
  return ix;
}
