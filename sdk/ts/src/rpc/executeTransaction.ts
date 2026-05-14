import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

/**
 *  Execute the multisig transaction.
 *  The transaction must be `ExecuteReady`.
 */
export async function executeTransaction({
  connection,
  feePayer,
  settingsPda,
  transactionIndex,
  signer,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = await transactions.executeTransaction({
    connection,
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    transactionIndex,
    signer,
    programId,
  });

  tx.sign([feePayer, ...(signers ?? [])]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
