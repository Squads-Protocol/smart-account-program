import { PublicKey } from "@solana/web3.js";
import { createSetTimeLockAsAuthorityInstruction } from "../generated";

export function setTimeLockAsAuthority({
  settingsPda,
  settingsAuthority,
  timeLock,
  memo,
  programId,
}: {
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  timeLock: number;
  memo?: string;
  programId?: PublicKey;
}) {
  return createSetTimeLockAsAuthorityInstruction(
    {
      settings: settingsPda,
      settingsAuthority,
    },
    {
      args: {
        timeLock,
        memo: memo ?? null,
      },
    },
    programId
  );
}
