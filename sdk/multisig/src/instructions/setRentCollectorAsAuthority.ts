import { PublicKey, SystemProgram } from "@solana/web3.js";
import { createSetRentCollectorAsAuthorityInstruction } from "../generated";

export function setRentCollectorAsAuthority({
  settingsPda,
  settingsAuthority,
  newRentCollector,
  rentPayer,
  memo,
  programId,
}: {
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  newRentCollector: PublicKey | null;
  rentPayer: PublicKey;
  memo?: string;
  programId?: PublicKey;
}) {
  return createSetRentCollectorAsAuthorityInstruction(
    {
      settings: settingsPda,
      settingsAuthority,
      rentPayer,
      systemProgram: SystemProgram.programId,
    },
    {
      args: {
        rentCollector: newRentCollector,
        memo: memo ?? null,
      },
    },
    programId
  );
}
