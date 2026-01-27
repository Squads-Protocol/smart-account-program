import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";
import { ClientDataJsonReconstructionParams } from "../generated";

export async function createTransactionBufferV2({
  connection,
  feePayer,
  consensusAccount,
  transactionBuffer,
  rentPayer,
  bufferIndex,
  accountIndex,
  finalBufferHash,
  finalBufferSize,
  buffer,
  creatorKey,
  clientDataParams,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  consensusAccount: PublicKey;
  transactionBuffer: PublicKey;
  rentPayer: Signer;
  bufferIndex: number;
  accountIndex: number;
  finalBufferHash: number[];
  finalBufferSize: number;
  buffer: Uint8Array;
  creatorKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createTransactionBufferV2({
    blockhash,
    feePayer: feePayer.publicKey,
    consensusAccount,
    transactionBuffer,
    rentPayer: rentPayer.publicKey,
    bufferIndex,
    accountIndex,
    finalBufferHash,
    finalBufferSize,
    buffer,
    creatorKey,
    clientDataParams,
    programId,
  });

  const allSigners = [feePayer];
  if (rentPayer !== feePayer) allSigners.push(rentPayer);
  if (signers) allSigners.push(...signers);

  tx.sign(allSigners);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
