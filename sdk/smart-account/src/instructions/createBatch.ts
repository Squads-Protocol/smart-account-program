import { PublicKey } from "@solana/web3.js";
import { createCreateBatchInstruction, PROGRAM_ID } from "../generated";
import { getTransactionPda } from "../pda";

export function createBatch({
  settingsPda,
  creator,
  rentPayer,
  batchIndex,
  accountIndex,
  memo,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  /** Member of the multisig that is creating the batch. */
  creator: PublicKey;
  /** Payer for the batch account rent. If not provided, `creator` is used. */
  rentPayer?: PublicKey;
  batchIndex: bigint;
  accountIndex: number;
  memo?: string;
  programId?: PublicKey;
}) {
  const [batchPda] = getTransactionPda({
    settingsPda,
    transactionIndex: batchIndex,
    programId,
  });

  return createCreateBatchInstruction(
    {
      consensusAccount: settingsPda,
      creator,
      rentPayer: rentPayer ?? creator,
      batch: batchPda,
    },
    { args: { accountIndex, memo: memo ?? null } },
    programId
  );
}
