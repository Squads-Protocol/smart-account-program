import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import { translateAndThrowAnchorError } from "../errors";
import * as transactions from "../transactions";

export async function createProposalV2({
  connection,
  feePayer,
  creator,
  rentPayer,
  settingsPda,
  transactionIndex,
  isDraft,
  extraVerificationData,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  creator: Signer;
  rentPayer?: Signer;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  isDraft?: boolean;
  extraVerificationData?: Uint8Array | null;
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createProposalV2({
    blockhash,
    feePayer: feePayer.publicKey,
    rentPayer: rentPayer?.publicKey,
    settingsPda,
    transactionIndex,
    creator: creator.publicKey,
    isDraft,
    extraVerificationData,
    programId,
  });

  const allSigners = [feePayer, creator];
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
