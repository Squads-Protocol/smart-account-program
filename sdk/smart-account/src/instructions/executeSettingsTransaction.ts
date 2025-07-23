import { AccountMeta, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  createExecuteSettingsTransactionInstruction,
  PROGRAM_ID,
} from "../generated";
import { getProposalPda, getTransactionPda } from "../pda";

export function executeSettingsTransaction({
  settingsPda,
  transactionIndex,
  signer,
  rentPayer,
  spendingLimits,
  policies,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  rentPayer?: PublicKey;
  /** In case the transaction adds or removes SpendingLimits, pass the array of their Pubkeys here. */
  spendingLimits?: PublicKey[];
  policies?: PublicKey[];
  programId?: PublicKey;
}) {
  const [proposalPda] = getProposalPda({
    settingsPda,
    transactionIndex,
    programId,
  });
  const [transactionPda] = getTransactionPda({
    settingsPda,
    transactionIndex: transactionIndex,
    programId,
  });

  let remainingAccounts: AccountMeta[] = [];
  if (spendingLimits) {
    remainingAccounts = spendingLimits.map((spendingLimit) => ({
      pubkey: spendingLimit,
      isWritable: true,
      isSigner: false,
    }));
  }
  if (policies) {
    remainingAccounts = policies.map((policy) => ({
      pubkey: policy,
      isWritable: true,
      isSigner: false,
    }));
  }
  return createExecuteSettingsTransactionInstruction(
    {
      settings: settingsPda,
      signer: signer,
      proposal: proposalPda,
      transaction: transactionPda,
      rentPayer: rentPayer ?? signer,
      systemProgram: SystemProgram.programId,
      anchorRemainingAccounts: remainingAccounts,
    },
    programId
  );
}
