import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

export function extendTransactionBufferV2({
  blockhash,
  feePayer,
  settingsPda,
  transactionBuffer,
  creator,
  buffer,
  extraVerificationData,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  transactionBuffer: PublicKey;
  creator: PublicKey;
  buffer: Uint8Array;
  extraVerificationData?: Uint8Array | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.extendTransactionBufferV2({
        settingsPda,
        transactionBuffer,
        creator,
        buffer,
        extraVerificationData,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
