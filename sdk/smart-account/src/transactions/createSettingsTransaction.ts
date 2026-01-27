import {
  AccountMeta,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { SettingsAction } from "../generated";
import * as instructions from "../instructions";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `creator` and `feePayer` before sending it.
 */
export function createSettingsTransaction({
  blockhash,
  feePayer,
  creator,
  rentPayer,
  settingsPda,
  transactionIndex,
  actions,
  memo,
  remainingAccounts,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  /** Member of the multisig that is creating the transaction. */
  creator: PublicKey;
  /** Payer for the transaction account rent. If not provided, `creator` is used. */
  rentPayer?: PublicKey;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  actions: SettingsAction[];
  memo?: string;
  remainingAccounts?: AccountMeta[];
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.createSettingsTransaction({
        creator,
        rentPayer,
        settingsPda,
        transactionIndex,
        actions,
        memo,
        remainingAccounts,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
