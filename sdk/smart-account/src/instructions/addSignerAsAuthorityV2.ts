import { PublicKey, SystemProgram } from "@solana/web3.js";
import {
  createAddSignerAsAuthorityV2Instruction,
  Permissions,
  PROGRAM_ID,
} from "../generated";

export function addSignerAsAuthorityV2({
  settingsPda,
  settingsAuthority,
  rentPayer,
  signerType,
  key,
  permissions,
  signerData,
  memo,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  rentPayer: PublicKey;
  signerType: number;
  key: PublicKey;
  permissions: Permissions;
  signerData: Uint8Array;
  memo?: string;
  programId?: PublicKey;
}) {
  return createAddSignerAsAuthorityV2Instruction(
    {
      settings: settingsPda,
      settingsAuthority,
      rentPayer,
      systemProgram: SystemProgram.programId,
      program: programId,
    },
    {
      args: {
        signerType,
        key,
        permissions,
        signerData,
        memo: memo ?? null,
      },
    },
    programId
  );
}
