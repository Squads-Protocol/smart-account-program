import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function createTransactionFromBufferV2({
  connection,
  feePayer,
  settingsPda,
  creator,
  rentPayer,
  transactionBuffer,
  transactionIndex,
  accountIndex,
  ephemeralSigners,
  memo,
  extraVerificationData,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  creator: PublicKey;
  rentPayer?: PublicKey;
  transactionBuffer: PublicKey;
  transactionIndex: bigint;
  accountIndex: number;
  ephemeralSigners: number;
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createTransactionFromBufferV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    creator,
    rentPayer,
    transactionBuffer,
    transactionIndex,
    accountIndex,
    ephemeralSigners,
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
