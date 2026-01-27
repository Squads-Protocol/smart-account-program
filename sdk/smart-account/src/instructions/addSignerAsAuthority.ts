import { PublicKey, SystemProgram } from "@solana/web3.js";
import {
  createAddSignerAsAuthorityInstruction,
  LegacySmartAccountSigner,
  PROGRAM_ID,
} from "../generated";

export function addSignerAsAuthority({
  settingsPda,
  settingsAuthority,
  rentPayer,
  newSigner,
  memo,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  rentPayer: PublicKey;
  newSigner: LegacySmartAccountSigner;
  memo?: string;
  programId?: PublicKey;
}) {
  return createAddSignerAsAuthorityInstruction(
    {
      settings: settingsPda,
      settingsAuthority,
      rentPayer,
      systemProgram: SystemProgram.programId,
      program: programId,
    },
    { args: { newSigner, memo: memo ?? null } },
    programId
  );
}
