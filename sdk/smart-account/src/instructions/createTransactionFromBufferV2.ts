import { PublicKey, SystemProgram } from "@solana/web3.js";
import { createCreateTransactionFromBufferV2Instruction, PROGRAM_ID } from "../generated";
import { getTransactionPda } from "../pda";
import { patchInstructionEvd } from "../utils";

export function createTransactionFromBufferV2({
  settingsPda,
  creator,
  rentPayer,
  transactionBuffer,
  transactionIndex,
  accountIndex,
  ephemeralSigners,
  memo,
  extraVerificationData,
  isExternalSigner,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  creator: PublicKey;
  rentPayer?: PublicKey;
  transactionBuffer: PublicKey;
  transactionIndex: bigint;
  accountIndex: number;
  ephemeralSigners: number;
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  isExternalSigner?: boolean;
  programId?: PublicKey;
}) {
  const [transactionPda] = getTransactionPda({
    settingsPda,
    transactionIndex,
    programId,
  });

  const ix = createCreateTransactionFromBufferV2Instruction(
    {
      transactionCreateItemConsensusAccount: settingsPda,
      transactionCreateItemTransaction: transactionPda,
      transactionCreateItemCreator: creator,
      transactionCreateItemRentPayer: rentPayer ?? creator,
      transactionCreateItemSystemProgram: SystemProgram.programId,
      transactionCreateItemProgram: programId,
      transactionBuffer,
      creator,
    },
    {
      args: {
        __kind: "TransactionPayload",
        fields: [
          {
            accountIndex,
            ephemeralSigners,
            transactionMessage: new Uint8Array(0),
            memo: memo ?? null,
          },
        ],
      },
      extraVerificationData: null,
    },
    programId
  );
  if (extraVerificationData && extraVerificationData.length > 0) {
    patchInstructionEvd(ix, extraVerificationData);
  } else if (!isExternalSigner) {
    const creatorMeta = [...ix.keys].reverse().find((k) => k.pubkey.equals(creator));
    if (creatorMeta) creatorMeta.isSigner = true;
  }
  return ix;
}
