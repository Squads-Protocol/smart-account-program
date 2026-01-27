import {
  AddressLookupTableAccount,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions/index";
import { PolicyPayload } from "../generated";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `creator`, `rentPayer` and `feePayer` before sending it.
 */
export function createPolicyTransaction({
  blockhash,
  feePayer,
  policy,
  transactionIndex,
  creator,
  rentPayer,
  accountIndex,
  policyPayload,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  policy: PublicKey;
  transactionIndex: bigint;
  /** Member of the multisig that is creating the transaction. */
  creator: PublicKey;
  /** Payer for the transaction account rent. If not provided, `creator` is used. */
  rentPayer?: PublicKey;
  accountIndex: number;
  policyPayload: PolicyPayload;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.createPolicyTransaction({
        policy,
        transactionIndex,
        creator,
        rentPayer,
        accountIndex,
        policyPayload,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
