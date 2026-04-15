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
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  signer: Signer;
  rentPayer?: Signer;
  accountIndex: number;
  batchIndex: bigint;
  transactionIndex: number;
  ephemeralSigners: number;
  transactionMessage: TransactionMessage;
  addressLookupTableAccounts?: AddressLookupTableAccount[];
  extraVerificationData?: Uint8Array | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const tx = await transactions.addTransactionToBatchV2({
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
    extraVerificationData,
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
