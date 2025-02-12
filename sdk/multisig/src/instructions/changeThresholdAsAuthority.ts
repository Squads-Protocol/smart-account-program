import { PublicKey } from "@solana/web3.js";
import {
  createChangeThresholdAsAuthorityInstruction,
  PROGRAM_ID,
} from "../generated";

export function changeThresholdAsAuthority({
  settingsPda,
  settingsAuthority,
  rentPayer,
  newThreshold,
  memo,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  rentPayer: PublicKey;
  newThreshold: number;
  memo?: string;
  programId?: PublicKey;
}) {
  return createChangeThresholdAsAuthorityInstruction(
    {
      settings: settingsPda,
      settingsAuthority,
      rentPayer,
      program: programId,
    },
    {
      args: {
        newThreshold,
        memo: memo ?? null,
      },
    },
    programId
  );
}
