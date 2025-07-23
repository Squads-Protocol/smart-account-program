import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";
import { PolicyPayload } from "../generated";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by required signers and `feePayer` before sending it.
 */
export function executePolicyPayloadSync({
  blockhash,
  feePayer,
  policy,
  accountIndex,
  numSigners,
  policyPayload,
  instruction_accounts,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  policy: PublicKey;
  accountIndex: number;
  numSigners: number;
  policyPayload: PolicyPayload;
  instruction_accounts: {
    pubkey: PublicKey;
    isWritable: boolean;
    isSigner: boolean;
  }[];
  programId?: PublicKey;
}): VersionedTransaction {
  const instruction = instructions.executePolicyPayloadSync({
    policy,
    accountIndex,
    numSigners,
    policyPayload,
    instruction_accounts,
    programId,
  });

  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [instruction],
  }).compileToV0Message();

  

  return new VersionedTransaction(message);
}