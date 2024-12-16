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
        memo: memo ?? null,
      },
    },
    programId
  );
}
