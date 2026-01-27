import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions/index";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `signer` and `feePayer` before sending it.
 */
export function incrementAccountIndex({
  blockhash,
  feePayer,
  settingsPda,
  signer,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  signer: PublicKey;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.incrementAccountIndex({
        settingsPda,
        signer,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
