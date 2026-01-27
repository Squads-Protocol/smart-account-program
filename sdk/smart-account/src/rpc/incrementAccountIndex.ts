import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function incrementAccountIndex({
  connection,
  feePayer,
  settings,
  signer,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settings: PublicKey;
  signer: Signer;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.incrementAccountIndex({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda: settings,
    signer: signer.publicKey,
    programId,
  });

  const allSigners = [feePayer];
  if (signer !== feePayer) allSigners.push(signer);
  if (signers) allSigners.push(...signers);

  tx.sign(allSigners);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
