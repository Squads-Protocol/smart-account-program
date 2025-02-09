import {
  AccountMeta,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions/index";
import { SettingsAction } from "../generated";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `feePayer` before sending it.
 */
export function executeSettingsTransactionSync({
  blockhash,
  feePayer,
  settingsPda,
  signers,
  settingsActions,
  memo,
  programId,
  remainingAccounts,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  signers: PublicKey[];
  settingsActions: SettingsAction[];
  memo?: string;
  programId?: PublicKey;
  remainingAccounts?: AccountMeta[];
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.executeSettingsTransactionSync({
        settingsPda,
        feePayer,
        signers,
        actions: settingsActions,
        memo,
        programId,
        remainingAccounts
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
