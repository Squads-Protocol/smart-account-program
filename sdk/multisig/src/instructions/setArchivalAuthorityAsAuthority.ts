import { PublicKey, SystemProgram } from "@solana/web3.js";
import { createSetArchivalAuthorityAsAuthorityInstruction } from "../generated";

export function setArchivalAuthorityAsAuthority({
  settingsPda,
  settingsAuthority,
  newArchivalAuthority,
  memo,
  programId,
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
