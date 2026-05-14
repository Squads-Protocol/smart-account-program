import { PublicKey, SystemProgram } from "@solana/web3.js";
import BN from "bn.js";
import {
  createAddSpendingLimitAsAuthorityInstruction,
  Period,
  PROGRAM_ID,
} from "../generated";

export function addSpendingLimitAsAuthority({
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
  expiration,
  memo,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  spendingLimit: PublicKey;
  rentPayer: PublicKey;
  seed: PublicKey;
  accountIndex: number;
  mint: PublicKey;
  amount: bigint;
  period: Period;
  signers: PublicKey[];
  destinations: PublicKey[];
  expiration?: number;
  memo?: string;
  programId?: PublicKey;
}) {
  return createAddSpendingLimitAsAuthorityInstruction(
    {
      settings: settingsPda,
      settingsAuthority,
      rentPayer,
      systemProgram: SystemProgram.programId,
      spendingLimit,
      program: programId,
    },
    {
      args: {
        seed,
        accountIndex,
        mint,
        amount: new BN(amount.toString()),
        period,
        signers,
        destinations,
        expiration: expiration
          ? new BN(expiration)
          : new BN("9223372036854775807"),
        memo: memo ?? null,
      },
    },
    programId
  );
}
