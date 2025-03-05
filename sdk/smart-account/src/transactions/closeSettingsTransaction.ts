import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions/index.js";

export function closeSettingsTransaction({
  blockhash,
  feePayer,
  settingsPda,
  transactionRentCollector,
  transactionIndex,
  programId,
  proposalRentCollector = transactionRentCollector,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  transactionRentCollector: PublicKey;
  transactionIndex: bigint;
  programId?: PublicKey;
  proposalRentCollector?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.closeSettingsTransaction({
        settingsPda,
        transactionRentCollector,
        transactionIndex,
        programId,
        proposalRentCollector,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
