import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { Period } from "../generated";
import * as instructions from "../instructions/index";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `configAuthority` and `feePayer` before sending it.
 */
export function removeSpendingLimitAsAuthority({
  blockhash,
  feePayer,
  settingsPda,
  settingsAuthority,
  spendingLimit,
  rentCollector,
  memo,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  spendingLimit: PublicKey;
  settingsAuthority: PublicKey;
  rentCollector: PublicKey;
  memo?: string;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.removeSpendingLimitAsAuthority({
        settingsPda,
        settingsAuthority,
        spendingLimit,
        rentCollector,
        memo,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
