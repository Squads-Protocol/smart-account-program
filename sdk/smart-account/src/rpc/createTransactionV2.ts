import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";
import { ClientDataJsonReconstructionParams, CreateTransactionArgs } from "../generated";

export async function createTransactionV2({
  connection,
  feePayer,
  consensusAccount,
  transaction,
  rentPayer,
  createArgs,
  creatorKey,
  clientDataParams,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  consensusAccount: PublicKey;
  transaction: PublicKey;
  rentPayer: Signer;
  createArgs: CreateTransactionArgs;
  creatorKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createTransactionV2({
    blockhash,
    feePayer: feePayer.publicKey,
    consensusAccount,
    transaction,
    rentPayer: rentPayer.publicKey,
    createArgs,
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
