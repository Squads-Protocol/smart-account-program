import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import { translateAndThrowAnchorError } from "../errors.js";
import * as transactions from "../transactions/index.js";

/**
 * Close the Proposal and ConfigTransaction accounts associated with a config transaction.
 */
export async function closeSettingsTransaction({
  connection,
  feePayer,
  settingsPda,
  rentCollector,
  transactionIndex,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  rentCollector: PublicKey;
  transactionIndex: bigint;
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.closeSettingsTransaction({
    blockhash,
    feePayer: feePayer.publicKey,
    rentCollector,
    transactionIndex,
    settingsPda,
    programId,
  });

  tx.sign([feePayer]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
