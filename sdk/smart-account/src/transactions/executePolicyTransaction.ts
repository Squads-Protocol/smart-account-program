import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
  AccountMeta,
} from "@solana/web3.js";
import * as instructions from "../instructions";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `member` and `feePayer` before sending it.
 */
export function executePolicyTransaction({
  blockhash,
  feePayer,
  policy,
  transactionIndex,
  signer,
  anchorRemainingAccounts,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  policy: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  anchorRemainingAccounts: AccountMeta[];
  programId?: PublicKey;
}): VersionedTransaction {
  const instruction = instructions.executePolicyTransaction({
    policy,
    signer,
    transactionIndex,
    anchorRemainingAccounts,
    programId,
  });

  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [instruction],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}