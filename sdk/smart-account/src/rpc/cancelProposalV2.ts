import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import { translateAndThrowAnchorError } from "../errors";
import * as transactions from "../transactions";

export async function cancelProposalV2({
  connection,
  feePayer,
  signer,
  settingsPda,
  transactionIndex,
  memo,
  extraVerificationData,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  signer: Signer;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.cancelProposalV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    transactionIndex,
    signer: signer.publicKey,
    memo,
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
