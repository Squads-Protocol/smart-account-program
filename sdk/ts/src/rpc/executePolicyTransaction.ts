import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
  AccountMeta,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

/**
 *  Execute the policy transaction.
 *  The transaction must be `ExecuteReady`.
 */
export async function executePolicyTransaction({
  connection,
  feePayer,
  policy,
  transactionIndex,
  signer,
  anchorRemainingAccounts,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  policy: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  anchorRemainingAccounts: AccountMeta[];
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.executePolicyTransaction({
    blockhash,
    feePayer: feePayer.publicKey,
    policy,
    transactionIndex,
    signer,
    anchorRemainingAccounts,
    programId,
  });

  tx.sign([feePayer, ...(signers ?? [])]);

  try {
    return await connection.sendRawTransaction(tx.serialize(), sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
