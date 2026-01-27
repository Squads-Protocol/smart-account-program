import {
  AccountMeta,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createExecuteSettingsTransactionV2Instruction, PROGRAM_ID, ClientDataJsonReconstructionParams } from "../generated";

export function executeSettingsTransactionV2({
  blockhash,
  feePayer,
  settings,
  proposal,
  transaction,
  executorKey,
  clientDataParams = null,
  remainingAccounts,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settings: PublicKey;
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
      createExecuteSettingsTransactionV2Instruction(
        {
          settings,
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
