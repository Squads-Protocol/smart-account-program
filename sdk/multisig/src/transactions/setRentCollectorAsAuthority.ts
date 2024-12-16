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
export function setRentCollectorAsAuthority({
  blockhash,
  feePayer,
  settingsPda,
  settingsAuthority,
  newRentCollector,
  rentPayer,
  memo,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  newRentCollector: PublicKey | null;
  rentPayer: PublicKey;
  memo?: string;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.setRentCollectorAsAuthority({
        settingsPda,
        settingsAuthority,
        newRentCollector,
        rentPayer,
        memo,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
