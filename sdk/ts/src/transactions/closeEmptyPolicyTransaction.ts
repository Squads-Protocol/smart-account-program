import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions/index.js";

export function closeEmptyPolicyTransaction({
  blockhash,
  feePayer,
  emptyPolicy,
  transactionRentCollector,
  transactionIndex,
  programId,
  proposalRentCollector = transactionRentCollector,
}: {
  blockhash: string;
  feePayer: PublicKey;
  emptyPolicy: PublicKey;
  transactionRentCollector: PublicKey;
  transactionIndex: bigint;
  programId?: PublicKey;
  proposalRentCollector?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.closeEmptyPolicyTransaction({
        emptyPolicy,
        transactionRentCollector,
        transactionIndex,
        programId,
        proposalRentCollector,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
