import {
  Connection,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

export async function executeBatchTransactionV2({
  connection,
  blockhash,
  feePayer,
  settingsPda,
  signer,
  batchIndex,
  transactionIndex,
  extraVerificationData,
  programId,
}: {
  connection: Connection;
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  signer: PublicKey;
  batchIndex: bigint;
  transactionIndex: number;
  extraVerificationData?: Uint8Array | null;
  programId?: PublicKey;
}): Promise<VersionedTransaction> {
  const { instruction, lookupTableAccounts } =
    await instructions.executeBatchTransactionV2({
      connection,
      settingsPda,
      signer,
      batchIndex,
      transactionIndex,
      extraVerificationData,
      programId,
    });

  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [instruction],
  }).compileToV0Message(lookupTableAccounts);

  return new VersionedTransaction(message);
}
