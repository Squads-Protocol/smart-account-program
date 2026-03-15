import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function createTransactionBufferV2({
  connection,
  feePayer,
  settingsPda,
  transactionBuffer,
  creator,
  rentPayer,
  bufferIndex,
  accountIndex,
  finalBufferHash,
  finalBufferSize,
  buffer,
  extraVerificationData,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  transactionBuffer: PublicKey;
  creator: PublicKey;
  rentPayer?: PublicKey;
  bufferIndex: number;
  accountIndex: number;
  finalBufferHash: number[];
  finalBufferSize: number;
  buffer: Uint8Array;
  extraVerificationData?: Uint8Array | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createTransactionBufferV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    transactionBuffer,
    creator,
    rentPayer,
    bufferIndex,
    accountIndex,
    finalBufferHash,
    finalBufferSize,
    buffer,
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
