import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

export function cancelProposalV2({
  blockhash,
  feePayer,
  settingsPda,
  transactionIndex,
  signer,
  memo,
  extraVerificationData,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.cancelProposalV2({
        signer,
        settingsPda,
        transactionIndex,
        memo,
        extraVerificationData,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
