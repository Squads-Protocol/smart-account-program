import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions/index.js";

export function closeTransaction({
  blockhash,
  feePayer,
  settingsPda,
  rentCollector,
  transactionIndex,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  rentCollector: PublicKey;
  transactionIndex: bigint;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.closeTransaction({
        settingsPda,
        rentCollector,
        transactionIndex,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
