import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function createSessionKey({
  connection,
  feePayer,
  signer,
  settingsPda,
  args,
  extraVerificationData,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  signer: Signer;
  settingsPda: PublicKey;
  args: {
    sessionKey: PublicKey;
    sessionKeyExpiration: bigint | number;
  };
  extraVerificationData: Uint8Array | null;
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createSessionKey({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    signer: signer.publicKey,
    args,
    extraVerificationData,
    programId,
  });

  tx.sign([feePayer, signer]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
