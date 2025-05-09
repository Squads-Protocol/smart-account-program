import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";

import * as instructions from "../instructions/index.js";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `member` and `feePayer` before sending it.
 */
export function createProposal({
  blockhash,
  feePayer,
  settingsPda,
  transactionIndex,
  creator,
  rentPayer,
  isDraft,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  /** Member of the multisig that is creating the proposal. */
  creator: PublicKey;
  /** Payer for the proposal account rent. If not provided, `creator` is used. */
  rentPayer?: PublicKey;
  isDraft?: boolean;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.createProposal({
        settingsPda,
        creator,
        rentPayer,
        transactionIndex,
        isDraft,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
