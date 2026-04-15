import {
  Connection,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

export async function executeTransactionV2({
  connection,
  blockhash,
  feePayer,
  settingsPda,
  transactionIndex,
  signer,
  extraVerificationData,
  programId,
}: {
  connection: Connection;
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  extraVerificationData?: Uint8Array | null;
  programId?: PublicKey;
}): Promise<VersionedTransaction> {
  const { instruction, lookupTableAccounts } =
    await instructions.executeTransactionV2({
      connection,
      settingsPda,
      signer,
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
