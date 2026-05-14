import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions/index.js";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `signer` and `feePayer` before sending it.
 */
export function activateProposal({
  blockhash,
  feePayer,
  settingsPda,
  transactionIndex,
  signer,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.activateProposal({
        settingsPda,
        transactionIndex,
        signer,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}