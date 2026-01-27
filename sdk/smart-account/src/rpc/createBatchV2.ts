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

export async function createBatchV2({
  connection,
  feePayer,
  settings,
  batch,
  creator,
  rentPayer,
  accountIndex,
  creatorKey,
  memo,
  clientDataParams,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settings: PublicKey;
  batch: PublicKey;
  creator: Signer;
  rentPayer: Signer;
  accountIndex: number;
  creatorKey: PublicKey;
  memo?: string | null;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createBatchV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settings,
    batch,
    creator: creator.publicKey,
    rentPayer: rentPayer.publicKey,
    accountIndex,
    creatorKey,
    memo,
    clientDataParams,
    programId,
  });

  const allSigners = [feePayer];
  if (creator !== feePayer) allSigners.push(creator);
  if (rentPayer !== feePayer && rentPayer !== creator) allSigners.push(rentPayer);
  if (signers) allSigners.push(...signers);

  tx.sign(allSigners);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
