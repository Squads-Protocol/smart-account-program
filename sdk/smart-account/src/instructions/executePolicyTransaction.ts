import {
  PublicKey,
  TransactionInstruction,
  AccountMeta,
} from "@solana/web3.js";
import {
  createExecuteTransactionInstruction,
  PROGRAM_ID,
} from "../generated";
import { getProposalPda, getTransactionPda } from "../pda";

export function executePolicyTransaction({
  policy,
  transactionIndex,
  signer,
  anchorRemainingAccounts,
  programId = PROGRAM_ID,
}: {
  policy: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  anchorRemainingAccounts: AccountMeta[];
  programId?: PublicKey;
}): TransactionInstruction {
  const [proposalPda] = getProposalPda({
    settingsPda: policy,
    transactionIndex,
    programId,
  });
  const [transactionPda] = getTransactionPda({
    settingsPda: policy,
    transactionIndex,
    programId,
  });

  return createExecuteTransactionInstruction(
    {
      consensusAccount: policy,
      signer,
      proposal: proposalPda,
      transaction: transactionPda,
      anchorRemainingAccounts,
      program: programId,
    },
    programId
  );
}