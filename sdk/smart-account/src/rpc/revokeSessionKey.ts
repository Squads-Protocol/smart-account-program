import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function revokeSessionKey({
  connection,
  feePayer,
  authority,
  settingsPda,
  signerKey,
  extraVerificationData,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  authority: Signer;
  settingsPda: PublicKey;
  signerKey: PublicKey;
  extraVerificationData?: Uint8Array | null;
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.revokeSessionKey({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    authority: authority.publicKey,
    signerKey,
    extraVerificationData,
    programId,
  });

  tx.sign([feePayer, authority]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
