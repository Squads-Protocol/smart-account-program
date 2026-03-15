import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

export function executeSettingsTransactionV2({
  blockhash,
  feePayer,
  settingsPda,
  signer,
  rentPayer,
  transactionIndex,
  spendingLimits,
  policies,
  extraVerificationData,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  rentPayer: PublicKey;
  spendingLimits?: PublicKey[];
  policies?: PublicKey[];
  extraVerificationData?: Uint8Array | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.executeSettingsTransactionV2({
        settingsPda,
        transactionIndex,
        signer,
        rentPayer,
        spendingLimits,
        policies,
        extraVerificationData,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
