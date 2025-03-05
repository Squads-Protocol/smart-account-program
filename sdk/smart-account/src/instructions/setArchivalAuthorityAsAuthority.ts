import { PublicKey, SystemProgram } from "@solana/web3.js";
import { createSetArchivalAuthorityAsAuthorityInstruction, PROGRAM_ID } from "../generated";

export function setArchivalAuthorityAsAuthority({
  settingsPda,
  settingsAuthority,
  newArchivalAuthority,
  memo,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  newArchivalAuthority: PublicKey | null;
  memo?: string;
  programId?: PublicKey;
}) {
  return createSetArchivalAuthorityAsAuthorityInstruction(
    {
      settings: settingsPda,
      settingsAuthority,
      systemProgram: SystemProgram.programId,
      program: programId,
    },
    {
      args: {
        newArchivalAuthority: newArchivalAuthority,
        memo: memo ?? null,
      },
    },
    programId
  );
}
