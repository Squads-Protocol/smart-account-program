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
  transactionRentCollector,
  transactionIndex,
  sendOptions,
  programId,
  proposalRentCollector,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  transactionRentCollector: PublicKey;
  transactionIndex: bigint;
  sendOptions?: SendOptions;
  programId?: PublicKey;
  proposalRentCollector?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.closeSettingsTransaction({
    blockhash,
    feePayer: feePayer.publicKey,
    transactionRentCollector,
    transactionIndex,
    settingsPda,
    programId,
    proposalRentCollector,
  });

  tx.sign([feePayer]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
