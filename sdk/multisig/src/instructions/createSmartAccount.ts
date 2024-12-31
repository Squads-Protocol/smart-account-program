import { PublicKey, TransactionInstruction } from "@solana/web3.js";
import {
  createCreateSmartAccountInstruction,
  SmartAccountSigner,
  PROGRAM_ID,
} from "../generated";
import { getProgramConfigPda } from "../pda";

export function createSmartAccount({
  treasury,
  creator,
  settings,
  settingsAuthority,
  threshold,
  signers,
  timeLock,
  rentCollector,
  memo,
  programId = PROGRAM_ID,
}: {
  treasury: PublicKey;
  creator: PublicKey;
  settings: PublicKey;
  settingsAuthority: PublicKey | null;
  threshold: number;
  signers: SmartAccountSigner[];
  timeLock: number;
  rentCollector: PublicKey | null;
  memo?: string;
  programId?: PublicKey;
}): TransactionInstruction {
  const programConfigPda = getProgramConfigPda({ programId })[0];

  return createCreateSmartAccountInstruction(
    {
      programConfig: programConfigPda,
      treasury,
      creator,
      settings: settings,
    },
    {
      args: {
        settingsAuthority,
        threshold,
        signers,
        timeLock,
        rentCollector,
        memo: memo ?? null,
      },
    },
    programId
  );
}
