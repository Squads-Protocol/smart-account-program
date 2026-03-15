import {
  AddressLookupTableAccount,
  Connection,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

export async function addTransactionToBatchV2({
  connection,
  feePayer,
  settingsPda,
  signer,
  rentPayer,
  accountIndex,
  batchIndex,
  transactionIndex,
  ephemeralSigners,
  transactionMessage,
  addressLookupTableAccounts,
  extraVerificationData,
  programId,
}: {
  connection: Connection;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  signer: PublicKey;
  rentPayer?: PublicKey;
  accountIndex: number;
  batchIndex: bigint;
  transactionIndex: number;
  ephemeralSigners: number;
  transactionMessage: TransactionMessage;
  addressLookupTableAccounts?: AddressLookupTableAccount[];
  extraVerificationData?: Uint8Array | null;
  programId?: PublicKey;
}): Promise<VersionedTransaction> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.addTransactionToBatchV2({
        accountIndex,
        settingsPda,
        signer,
        rentPayer,
        batchIndex,
        transactionIndex,
        ephemeralSigners,
        transactionMessage,
        addressLookupTableAccounts,
        extraVerificationData,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
