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

export async function extendTransactionBufferV2({
  connection,
  feePayer,
  consensusAccount,
  transactionBuffer,
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
  buffer: Uint8Array;
  creatorKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.extendTransactionBufferV2({
    blockhash,
    feePayer: feePayer.publicKey,
    consensusAccount,
    transactionBuffer,
    buffer,
    creatorKey,
    clientDataParams,
    programId,
  });

  tx.sign([feePayer, ...(signers ?? [])]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
