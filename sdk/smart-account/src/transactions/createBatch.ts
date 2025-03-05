import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `creator` and `feePayer` before sending it.
 */
export function createBatch({
  blockhash,
  feePayer,
  settingsPda,
  batchIndex,
  creator,
  rentPayer,
  accountIndex,
  memo,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  batchIndex: bigint;
  /** Member of the multisig that is creating the batch. */
  creator: PublicKey;
  /** Payer for the batch account rent. If not provided, `creator` is used. */
  rentPayer?: PublicKey;
  accountIndex: number;
  memo?: string;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.createBatch({
        settingsPda,
        creator,
        rentPayer: rentPayer ?? creator,
        batchIndex,
        accountIndex,
        memo,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
