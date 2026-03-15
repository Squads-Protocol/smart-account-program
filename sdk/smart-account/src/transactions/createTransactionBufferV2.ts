import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

export function createTransactionBufferV2({
  blockhash,
  feePayer,
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
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
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
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.createTransactionBufferV2({
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
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
