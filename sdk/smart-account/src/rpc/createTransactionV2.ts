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

export async function createTransactionV2({
  connection,
  feePayer,
  settingsPda,
  transactionIndex,
  creator,
  rentPayer,
  accountIndex,
  ephemeralSigners,
  transactionMessage,
  addressLookupTableAccounts,
  memo,
  extraVerificationData,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  creator: PublicKey;
  rentPayer?: PublicKey;
  accountIndex: number;
  ephemeralSigners: number;
  transactionMessage: TransactionMessage;
  addressLookupTableAccounts?: AddressLookupTableAccount[];
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createTransactionV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    transactionIndex,
    creator,
    rentPayer,
    accountIndex,
    ephemeralSigners,
    transactionMessage,
    addressLookupTableAccounts,
    memo,
    extraVerificationData,
    programId,
  });

  tx.sign([feePayer, ...(signers ?? [])]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
