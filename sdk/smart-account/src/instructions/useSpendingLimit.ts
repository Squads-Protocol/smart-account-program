import {
  PublicKey,
  SystemProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { createUseSpendingLimitInstruction, PROGRAM_ID } from "../generated";
import {  getSmartAccountPda } from "../pda";

export function useSpendingLimit({
  settingsPda,
  signer,
  spendingLimit,
  mint,
  accountIndex,
  amount,
  decimals,
  destination,
  tokenProgram = TOKEN_PROGRAM_ID,
  memo,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  signer: PublicKey;
  spendingLimit: PublicKey;
  /** Provide if `spendingLimit` is for an SPL token, omit if it's for SOL. */
  mint?: PublicKey;
  accountIndex: number;
  amount: number;
  decimals: number;
  destination: PublicKey;
  tokenProgram?: PublicKey;
  memo?: string;
  programId?: PublicKey;
}): TransactionInstruction {
  const [smartAccountPda] = getSmartAccountPda({ settingsPda, accountIndex, programId });

  const smartAccountTokenAccount =
    mint &&
    getAssociatedTokenAddressSync(
      mint,
      smartAccountPda,
      true,
      tokenProgram,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );

  const destinationTokenAccount =
    mint &&
    getAssociatedTokenAddressSync(
      mint,
      destination,
      true,
      tokenProgram,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );

  return createUseSpendingLimitInstruction(
    {
      settings: settingsPda,
      signer,
      spendingLimit,
      smartAccount: smartAccountPda,
      destination,
      systemProgram: SystemProgram.programId,
      mint,
      smartAccountTokenAccount,
      destinationTokenAccount,
      tokenProgram: mint ? tokenProgram : undefined,
      program: programId,
    },
    { args: { amount, decimals, memo: memo ?? null } },
    programId
  );
}
