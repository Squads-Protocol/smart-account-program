import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

/** Execute a transaction from a batch. */
export async function executeBatchTransaction({
  connection,
  feePayer,
  settingsPda,
  signer,
  batchIndex,
  transactionIndex,
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
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = await transactions.executeBatchTransaction({
    connection,
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    signer: signer.publicKey,
    batchIndex,
    transactionIndex,
    programId,
  });

  tx.sign([feePayer, signer, ...(signers ?? [])]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
