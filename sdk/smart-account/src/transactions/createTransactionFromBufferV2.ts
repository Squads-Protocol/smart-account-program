import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

export function createTransactionFromBufferV2({
  blockhash,
  feePayer,
  settingsPda,
  creator,
  rentPayer,
  transactionBuffer,
  transactionIndex,
  accountIndex,
  ephemeralSigners,
  memo,
  extraVerificationData,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  creator: PublicKey;
  rentPayer?: PublicKey;
  transactionBuffer: PublicKey;
  transactionIndex: bigint;
  accountIndex: number;
  ephemeralSigners: number;
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.createTransactionFromBufferV2({
        settingsPda,
        creator,
        rentPayer,
        transactionBuffer,
        transactionIndex,
        accountIndex,
        ephemeralSigners,
        memo,
        extraVerificationData,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
