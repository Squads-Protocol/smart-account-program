import {
  Connection,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `member` and `feePayer` before sending it.
 */
export async function executeTransaction({
  connection,
  blockhash,
  feePayer,
  settingsPda,
  transactionIndex,
  signer,
  programId,
}: {
  connection: Connection;
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  programId?: PublicKey;
}): Promise<VersionedTransaction> {
  const { instruction, lookupTableAccounts } =
    await instructions.executeTransaction({
      connection,
      settingsPda,
      signer,
      transactionIndex,
      programId,
    });

  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [instruction],
  }).compileToV0Message(lookupTableAccounts);

  return new VersionedTransaction(message);
}
