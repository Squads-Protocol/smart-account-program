import { PublicKey, SystemProgram } from "@solana/web3.js";
import {
  createRemoveSessionKeyInstruction,
  PROGRAM_ID,
} from "../generated";
import type { ClientDataJsonReconstructionParams } from "../generated";

export function removeSessionKey({
  settingsPda,
  parentSigner,
  rentPayer,
  parentSignerKey,
  clientDataParams,
  memo,
  anchorRemainingAccounts,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  parentSigner: PublicKey;
  rentPayer: PublicKey;
  parentSignerKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams;
  memo?: string;
  anchorRemainingAccounts?: Array<{
    pubkey: PublicKey;
    isSigner: boolean;
    isWritable: boolean;
  }>;
  programId?: PublicKey;
}) {
  return createRemoveSessionKeyInstruction(
    {
      settings: settingsPda,
      parentSigner,
      rentPayer,
      systemProgram: SystemProgram.programId,
      program: programId,
      anchorRemainingAccounts,
    },
    {
      args: {
        parentSignerKey,
        clientDataParams: clientDataParams ?? null,
        memo: memo ?? null,
      },
    },
    programId
  );
}
