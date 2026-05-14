import {
  AddressLookupTableAccount,
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionMessage,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";
import { PolicyPayload } from "../generated";

/** Create a new vault transaction. */
export async function createPolicyTransaction({
  connection,
  feePayer,
  policy,
  transactionIndex,
  creator,
  rentPayer,
  accountIndex,
  policyPayload,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  policy: PublicKey;
  transactionIndex: bigint;
  /** Member of the multisig that is creating the transaction. */
  creator: PublicKey;
  /** Payer for the transaction account rent. If not provided, `creator` is used. */
  rentPayer?: PublicKey;
  accountIndex: number;
  policyPayload: PolicyPayload;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createPolicyTransaction({
    blockhash,
    feePayer: feePayer.publicKey,
    policy,
    transactionIndex,
    creator,
    rentPayer,
    accountIndex,
    policyPayload,
    programId,
  });

  tx.sign([feePayer, ...(signers ?? [])]);

  try {
    return await connection.sendRawTransaction(tx.serialize(), sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
