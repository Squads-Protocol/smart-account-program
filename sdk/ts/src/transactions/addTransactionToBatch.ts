import {
  AddressLookupTableAccount,
  Connection,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `creator` and `feePayer` before sending it.
 */
export async function addTransactionToBatch({
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
  programId,
}: {
  connection: Connection;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  /** Member of the multisig that is creating the transaction. */
  signer: PublicKey;
  /** Payer for the transaction account rent. If not provided, `member` is used. */
  rentPayer?: PublicKey;
  accountIndex: number;
  batchIndex: bigint;
  transactionIndex: number;
  /** Number of additional signing PDAs required by the transaction. */
  ephemeralSigners: number;
  /** Transaction message to wrap into a batch transaction. */
  transactionMessage: TransactionMessage;
  /** `AddressLookupTableAccount`s referenced in `transaction_message`. */
  addressLookupTableAccounts?: AddressLookupTableAccount[];
  programId?: PublicKey;
}): Promise<VersionedTransaction> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.addTransactionToBatch({
        accountIndex,
        settingsPda,
        signer,
        rentPayer,
        batchIndex,
        transactionIndex,
        ephemeralSigners,
        transactionMessage,
        addressLookupTableAccounts,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
