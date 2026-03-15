import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function executeTransactionV2({
  connection,
  feePayer,
  settingsPda,
  transactionIndex,
  signer,
  extraVerificationData,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  extraVerificationData?: Uint8Array | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = await transactions.executeTransactionV2({
    connection,
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    transactionIndex,
    signer,
    extraVerificationData,
    programId,
  });

  tx.sign([feePayer, ...(signers ?? [])]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
