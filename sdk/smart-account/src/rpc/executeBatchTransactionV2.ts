import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function executeBatchTransactionV2({
  connection,
  feePayer,
  settingsPda,
  signer,
  batchIndex,
  transactionIndex,
  extraVerificationData,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  signer: Signer;
  batchIndex: bigint;
  transactionIndex: number;
  extraVerificationData?: Uint8Array | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = await transactions.executeBatchTransactionV2({
    connection,
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    signer: signer.publicKey,
    batchIndex,
    transactionIndex,
    extraVerificationData,
    programId,
  });

  tx.sign([feePayer, signer, ...(signers ?? [])]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
