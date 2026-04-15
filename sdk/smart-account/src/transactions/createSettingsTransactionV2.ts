import {
  AccountMeta,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { SettingsAction } from "../generated";
import * as instructions from "../instructions";

export function createSettingsTransactionV2({
  blockhash,
  feePayer,
  creator,
  rentPayer,
  settingsPda,
  transactionIndex,
  actions,
  memo,
  extraVerificationData,
  remainingAccounts,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  creator: PublicKey;
  rentPayer?: PublicKey;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  actions: SettingsAction[];
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  remainingAccounts?: AccountMeta[];
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.createSettingsTransactionV2({
        creator,
        rentPayer,
        settingsPda,
        transactionIndex,
        actions,
        memo,
        extraVerificationData,
        remainingAccounts,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
