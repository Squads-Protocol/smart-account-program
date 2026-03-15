import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

export function createBatchV2({
  blockhash,
  feePayer,
  settingsPda,
  batchIndex,
  creator,
  rentPayer,
  accountIndex,
  memo,
  extraVerificationData,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  batchIndex: bigint;
  creator: PublicKey;
  rentPayer?: PublicKey;
  accountIndex: number;
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.createBatchV2({
        settingsPda,
        creator,
        rentPayer: rentPayer ?? creator,
        batchIndex,
        accountIndex,
        memo,
        extraVerificationData,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
