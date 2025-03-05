import { PublicKey, SystemProgram } from "@solana/web3.js";
import {
  createRemoveSignerAsAuthorityInstruction,
  PROGRAM_ID,
} from "../generated";

export function removeSignerAsAuthority({
  settingsPda,
  settingsAuthority,
  oldSigner,
  memo,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  oldSigner: PublicKey;
  memo?: string;
  programId?: PublicKey;
}) {
  return createRemoveSignerAsAuthorityInstruction(
    {
      settings: settingsPda,
      settingsAuthority,
      systemProgram: SystemProgram.programId,
      program: programId,
    },
    { args: { oldSigner, memo: memo ?? null } },
    programId
  );
}
