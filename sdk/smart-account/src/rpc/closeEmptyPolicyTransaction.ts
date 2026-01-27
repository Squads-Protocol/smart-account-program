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
 * Close the Proposal and ConfigTransaction accounts associated with a
 * empty/deleted policy.
 */
export async function closeEmptyPolicyTransaction({
  connection,
  feePayer,
  emptyPolicy,
  transactionRentCollector,
  transactionIndex,
  sendOptions,
  programId,
  proposalRentCollector,
}: {
  connection: Connection;
  feePayer: Signer;
  emptyPolicy: PublicKey;
  transactionRentCollector: PublicKey;
  transactionIndex: bigint;
  sendOptions?: SendOptions;
  programId?: PublicKey;
  proposalRentCollector?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.closeEmptyPolicyTransaction({
    blockhash,
    feePayer: feePayer.publicKey,
    emptyPolicy,
    transactionRentCollector,
    transactionIndex,
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
