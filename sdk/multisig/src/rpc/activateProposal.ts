import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function activateProposal({
  connection,
  feePayer,
  signer,
  settingsPda,
  transactionIndex,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  signer: Signer;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.activateProposal({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    transactionIndex,
    signer: signer.publicKey,
    programId,
  });

  tx.sign([feePayer, signer]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}