import {
  AddressLookupTableAccount,
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionMessage,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

/** Add a transaction to a batch. */
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
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  /** Member of the multisig that is adding the transaction. */
  signer: Signer;
  /** Payer for the transaction account rent. If not provided, `member` is used. */
  rentPayer?: Signer;
  accountIndex: number;
  batchIndex: bigint;
  transactionIndex: number;
  /** Number of additional signing PDAs required by the transaction. */
  ephemeralSigners: number;
  /** Transaction message to wrap into a batch transaction. */
  transactionMessage: TransactionMessage;
  /** `AddressLookupTableAccount`s referenced in `transaction_message`. */
  addressLookupTableAccounts?: AddressLookupTableAccount[];
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const tx = await transactions.addTransactionToBatch({
    connection,
    feePayer: feePayer.publicKey,
    settingsPda,
    signer: signer.publicKey,
    rentPayer: rentPayer?.publicKey ?? signer.publicKey,
    accountIndex,
    batchIndex,
    transactionIndex,
    ephemeralSigners,
    transactionMessage,
    addressLookupTableAccounts,
    programId,
  });

  const allSigners = [feePayer, signer];
  if (signers) {
    allSigners.push(...signers);
  }
  if (rentPayer) {
    allSigners.push(rentPayer);
  }
  tx.sign(allSigners);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
