import * as instructions from "../instructions/index";
import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `member` and `feePayer` before sending it.
 */
export function useSpendingLimit({
  blockhash,
  feePayer,
  settingsPda,
  signer,
  spendingLimit,
  mint,
  accountIndex,
  amount,
  decimals,
  destination,
  tokenProgram,
  memo,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  signer: PublicKey;
  spendingLimit: PublicKey;
  /** Provide if `spendingLimit` is for an SPL token, omit if it's for SOL. */
  mint?: PublicKey;
  accountIndex: number;
  amount: number;
  decimals: number;
  destination: PublicKey;
  tokenProgram?: PublicKey;
  memo?: string;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.useSpendingLimit({
        settingsPda,
        signer,
        spendingLimit,
        mint,
        accountIndex,
        amount,
        decimals,
        destination,
        tokenProgram,
        memo,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
