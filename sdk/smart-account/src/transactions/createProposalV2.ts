import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

export function createProposalV2({
  blockhash,
  feePayer,
  settingsPda,
  transactionIndex,
  creator,
  rentPayer,
  isDraft,
  extraVerificationData,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  creator: PublicKey;
  rentPayer?: PublicKey;
  isDraft?: boolean;
  extraVerificationData?: Uint8Array | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.createProposalV2({
        settingsPda,
        creator,
        rentPayer,
        transactionIndex,
        isDraft,
        extraVerificationData,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
