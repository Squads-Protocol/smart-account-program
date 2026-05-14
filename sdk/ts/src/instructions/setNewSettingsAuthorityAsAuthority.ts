import { PublicKey } from "@solana/web3.js";
import { createSetNewSettingsAuthorityAsAuthorityInstruction, PROGRAM_ID } from "../generated";

export function setNewSettingsAuthorityAsAuthority({
  settingsPda,
  settingsAuthority,
  newSettingsAuthority,
  memo,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  newSettingsAuthority: PublicKey;
  memo?: string;
  programId?: PublicKey;
}) {
  return createSetNewSettingsAuthorityAsAuthorityInstruction(
    {
      settings: settingsPda,
      settingsAuthority,
      program: programId,
    },
    {
      args: {
        newSettingsAuthority,
        memo: memo ?? null,
      },
    },
    programId
  );
}
