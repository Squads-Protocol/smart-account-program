import { PublicKey, SystemProgram } from "@solana/web3.js";
import BN from "bn.js";
import {
  createRemoveSpendingLimitAsAuthorityInstruction,
  Period,
  PROGRAM_ID,
} from "../generated";

export function removeSpendingLimitAsAuthority({
  settingsPda,
  settingsAuthority,
  spendingLimit,
  rentCollector,
  memo,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  spendingLimit: PublicKey;
  rentCollector: PublicKey;
  memo?: string;
  programId?: PublicKey;
}) {
  return createRemoveSpendingLimitAsAuthorityInstruction(
    {
      settings: settingsPda,
      settingsAuthority,
      spendingLimit,
      rentCollector,
    },
    {
      args: {
        memo: memo ?? null,
      },
    },
    programId
  );
}
