import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `configAuthority` and `feePayer` before sending it.
 */
export function setNewSettingsAuthorityAsAuthority({
  blockhash,
  feePayer,
  settingsPda,
  settingsAuthority,
  newSettingsAuthority,
  memo,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  newSettingsAuthority: PublicKey;
  memo?: string;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.setNewSettingsAuthorityAsAuthority({
        settingsPda,
        settingsAuthority,
        newSettingsAuthority,
        memo,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
