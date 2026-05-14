import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

/** Create a new vault transactions batch. */
export async function createBatch({
  connection,
  feePayer,
  settingsPda,
  batchIndex,
  creator,
  rentPayer,
  accountIndex,
  memo,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  batchIndex: bigint;
  /** Member of the multisig that is creating the batch. */
  creator: Signer;
  /** Payer for the batch account rent. If not provided, `creator` is used. */
  rentPayer?: Signer;
  accountIndex: number;
  memo?: string;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createBatch({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    batchIndex,
    creator: creator.publicKey,
    rentPayer: rentPayer?.publicKey ?? creator.publicKey,
    accountIndex,
    memo,
    programId,
  });

  const allSigners = [feePayer, creator];
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
