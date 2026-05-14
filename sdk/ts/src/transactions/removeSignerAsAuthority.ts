import {
  PublicKey,
  SystemProgram,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions/index";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `configAuthority` and `feePayer` before sending it.
 */
export function removeSignerAsAuthority({
  blockhash,
  feePayer,
  settingsPda,
  settingsAuthority,
  oldSigner,
  memo,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  oldSigner: PublicKey;
  memo?: string;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.removeSignerAsAuthority({
        settingsPda,
        settingsAuthority,
        oldSigner,
        memo,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
