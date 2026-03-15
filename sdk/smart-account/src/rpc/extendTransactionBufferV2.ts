import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function extendTransactionBufferV2({
  connection,
  feePayer,
  settingsPda,
  transactionBuffer,
  creator,
  buffer,
  extraVerificationData,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  transactionBuffer: PublicKey;
  creator: PublicKey;
  buffer: Uint8Array;
  extraVerificationData?: Uint8Array | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.extendTransactionBufferV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    transactionBuffer,
    creator,
    buffer,
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
