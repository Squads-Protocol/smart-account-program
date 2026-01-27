import {
  AccountMeta,
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  createCreateSmartAccountV2Instruction,
  PROGRAM_ID,
  SmartAccountSigner,
} from "../generated";
import { getProgramConfigPda } from "../pda";

export function createSmartAccountV2({
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
  signers: SmartAccountSigner[];
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
  return createCreateSmartAccountV2Instruction(
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
