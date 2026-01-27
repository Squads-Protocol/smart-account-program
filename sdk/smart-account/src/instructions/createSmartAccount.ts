import {
  AccountMeta,
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  createCreateSmartAccountInstruction,
  PROGRAM_ID,
  LegacySmartAccountSigner,
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
  remainingAccounts,
}: {
  treasury: PublicKey;
  creator: PublicKey;
  settings?: PublicKey;
  settingsAuthority: PublicKey | null;
  threshold: number;
  signers: LegacySmartAccountSigner[];
  timeLock: number;
  rentCollector: PublicKey | null;
  memo?: string;
  programId?: PublicKey;
  remainingAccounts?: AccountMeta[];
}): TransactionInstruction {
  const programConfigPda = getProgramConfigPda({ programId })[0];
  const settingsAccountMeta: AccountMeta = {
    pubkey: settings ?? PublicKey.default,
    isSigner: false,
    isWritable: true,
  };
  return createCreateSmartAccountInstruction(
    {
      programConfig: programConfigPda,
      treasury,
      creator,
      program: programId,
      anchorRemainingAccounts: [
        settingsAccountMeta,
        ...(remainingAccounts ?? []),
      ],
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
