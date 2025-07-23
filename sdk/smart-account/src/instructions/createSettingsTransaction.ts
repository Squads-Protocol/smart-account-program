import { AccountMeta, PublicKey } from "@solana/web3.js";
import {
  SettingsAction,
  createCreateSettingsTransactionInstruction,
  PROGRAM_ID,
} from "../generated";
import { getTransactionPda } from "../pda";

export function createSettingsTransaction({
  settingsPda,
  transactionIndex,
  creator,
  rentPayer,
  actions,
  memo,
  remainingAccounts,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  /** Member of the multisig that is creating the transaction. */
  creator: PublicKey;
  /** Payer for the transaction account rent. If not provided, `creator` is used. */
  rentPayer?: PublicKey;
  transactionIndex: bigint;
  actions: SettingsAction[];
  memo?: string;
  remainingAccounts?: AccountMeta[];
  programId?: PublicKey;
}) {
  const [transactionPda] = getTransactionPda({
    settingsPda,
    transactionIndex: transactionIndex,
    programId,
  });

  return createCreateSettingsTransactionInstruction(
    {
      settings: settingsPda,
      transaction: transactionPda,
      creator,
      rentPayer: rentPayer ?? creator,
      anchorRemainingAccounts: remainingAccounts,
    },
    { args: { actions, memo: memo ?? null } },
    programId
  );
}
