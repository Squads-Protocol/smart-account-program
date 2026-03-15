import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function createBatchV2({
  connection,
  feePayer,
  settingsPda,
  batchIndex,
  creator,
  rentPayer,
  accountIndex,
  memo,
  extraVerificationData,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  batchIndex: bigint;
  creator: Signer;
  rentPayer?: Signer;
  accountIndex: number;
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createBatchV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    batchIndex,
    creator: creator.publicKey,
    rentPayer: rentPayer?.publicKey ?? creator.publicKey,
    accountIndex,
    memo,
    extraVerificationData,
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
