import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions/index";
import { ConfigAction } from "../generated";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `feePayer` before sending it.
 */
export function configTransactionSync({
  blockhash,
  feePayer,
  multisigPda,
  signers,
  configActions,
  memo,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  multisigPda: PublicKey;
  signers: PublicKey[];
  configActions: ConfigAction[];
  memo?: string;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.configTransactionSync({
        multisigPda,
        feePayer,
        signers,
        configActions,
        memo,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
