import { PublicKey, SystemProgram } from "@solana/web3.js";
import BN from "bn.js";
import {
  createAddSessionKeyInstruction,
  PROGRAM_ID,
} from "../generated";
import type { ClientDataJsonReconstructionParams } from "../generated";

export function addSessionKey({
  settingsPda,
  parentSigner,
  rentPayer,
  parentSignerKey,
  sessionKey,
  expiration,
  clientDataParams,
  memo,
  anchorRemainingAccounts,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  parentSigner: PublicKey;
  rentPayer: PublicKey;
  parentSignerKey: PublicKey;
  sessionKey: PublicKey;
  expiration: bigint | number;
  clientDataParams?: ClientDataJsonReconstructionParams;
  memo?: string;
  anchorRemainingAccounts?: Array<{
    pubkey: PublicKey;
    isSigner: boolean;
    isWritable: boolean;
  }>;
  programId?: PublicKey;
}) {
  return createAddSessionKeyInstruction(
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
        sessionKey,
        expiration: typeof expiration === "bigint" ? new BN(expiration.toString()) : new BN(expiration),
        clientDataParams: clientDataParams ?? null,
        memo: memo ?? null,
      },
    },
    programId
  );
}
