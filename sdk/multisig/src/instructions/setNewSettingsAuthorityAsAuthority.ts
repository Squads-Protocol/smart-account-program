import { PublicKey } from "@solana/web3.js";
import { createSetNewSettingsAuthorityAsAuthorityInstruction } from "../generated";

export function setNewSettingsAuthorityAsAuthority({
  settingsPda,
  settingsAuthority,
  newSettingsAuthority,
  memo,
  programId,
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
