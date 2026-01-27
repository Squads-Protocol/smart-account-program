import {
  AccountMeta,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createExecuteSettingsTransactionSyncV2Instruction, PROGRAM_ID, SettingsAction, ClientDataJsonReconstructionParams } from "../generated";

export function executeSettingsTransactionSyncV2({
  blockhash,
  feePayer,
  consensusAccount,
  numNativeSigners,
  externalSignerKeyIds,
  clientDataParams = null,
  actions,
  memo = null,
  remainingAccounts,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  consensusAccount: PublicKey;
  numNativeSigners: number;
  externalSignerKeyIds: PublicKey[];
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  actions: SettingsAction[];
  memo?: string | null;
  remainingAccounts?: AccountMeta[];
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createExecuteSettingsTransactionSyncV2Instruction(
        {
          consensusAccount,
          program: programId,
          anchorRemainingAccounts: remainingAccounts,
        },
        {
          args: {
            numNativeSigners,
            externalSignerKeyIds,
            clientDataParams,
            actions,
            memo,
          },
        },
        programId
      ),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
