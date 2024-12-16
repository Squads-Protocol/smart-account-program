import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { Period } from "../generated";
import * as instructions from "../instructions/index";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `configAuthority`, `rent_payer` and `feePayer` before sending it.
 */
export function addSpendingLimitAsAuthority({
  blockhash,
  feePayer,
  settingsPda,
  settingsAuthority,
  spendingLimit,
  rentPayer,
  seed,
  accountIndex,
  mint,
  amount,
  period,
  signers,
  destinations,
  memo,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  rentPayer: PublicKey;
  spendingLimit: PublicKey;
  seed: PublicKey;
  accountIndex: number;
  mint: PublicKey;
  amount: bigint;
  period: Period;
  signers: PublicKey[];
  destinations: PublicKey[];
  memo?: string;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
        instructions.addSpendingLimitAsAuthority({
        settingsPda,
        settingsAuthority,
        spendingLimit,
        rentPayer,
        seed,
        accountIndex,
        mint,
        amount,
        period,
        signers,
        destinations,
        memo,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
