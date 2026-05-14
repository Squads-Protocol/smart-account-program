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
  programId?: PublicKey;
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
        remainingAccounts,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
