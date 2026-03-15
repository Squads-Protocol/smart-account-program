import {
  AccountMeta,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";
import { SettingsAction } from "../generated";

export function executeSettingsTransactionSyncV2({
  blockhash,
  feePayer,
  settingsPda,
  signers,
  settingsActions,
  memo,
  extraVerificationData,
  remainingAccounts,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  signers: PublicKey[];
  settingsActions: SettingsAction[];
  remainingAccounts?: AccountMeta[];
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.executeSettingsTransactionSyncV2({
        settingsPda,
        feePayer,
        signers,
        actions: settingsActions,
        memo,
        extraVerificationData,
        remainingAccounts,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
