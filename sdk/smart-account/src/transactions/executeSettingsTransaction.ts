import {
  AccountMeta,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `creator` and `feePayer` before sending it.
 */
export function executeSettingsTransaction({
  blockhash,
  feePayer,
  settingsPda,
  signer,
  rentPayer,
  transactionIndex,
  spendingLimits,
  policies,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  rentPayer: PublicKey;
  /** In case the transaction adds or removes SpendingLimits, pass the array of their Pubkeys here. */
  spendingLimits?: PublicKey[];
  /** In case the transaction adds or removes Policies, pass the array of their Pubkeys here. */
  policies?: PublicKey[];
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.executeSettingsTransaction({
        settingsPda,
        transactionIndex,
        signer,
        rentPayer,
        spendingLimits,
        policies,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
