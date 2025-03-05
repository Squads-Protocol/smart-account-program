import { PublicKey } from "@solana/web3.js";
import { createSetTimeLockAsAuthorityInstruction, PROGRAM_ID } from "../generated";

export function setTimeLockAsAuthority({
  settingsPda,
  settingsAuthority,
  timeLock,
  memo,
  programId = PROGRAM_ID,
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
      program: programId,
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
