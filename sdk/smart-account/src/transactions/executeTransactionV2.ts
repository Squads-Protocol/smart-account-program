import {
  AccountMeta,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createExecuteTransactionV2Instruction, PROGRAM_ID, ClientDataJsonReconstructionParams } from "../generated";

export function executeTransactionV2({
  blockhash,
  feePayer,
  consensusAccount,
  proposal,
  transaction,
  executorKey,
  clientDataParams = null,
  remainingAccounts,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  consensusAccount: PublicKey;
  proposal: PublicKey;
  transaction: PublicKey;
  executorKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  remainingAccounts?: AccountMeta[];
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createExecuteTransactionV2Instruction(
        {
          consensusAccount,
          proposal,
          transaction,
          program: programId,
          anchorRemainingAccounts: remainingAccounts,
        },
        {
          args: {
            executorKey,
            clientDataParams,
          },
        },
        programId
      ),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
