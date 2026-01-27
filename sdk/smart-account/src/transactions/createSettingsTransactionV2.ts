import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createCreateSettingsTransactionV2Instruction, PROGRAM_ID, ClientDataJsonReconstructionParams, SettingsAction } from "../generated";

export function createSettingsTransactionV2({
  blockhash,
  feePayer,
  settings,
  transaction,
  rentPayer,
  actions,
  creatorKey,
  clientDataParams = null,
  memo = null,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settings: PublicKey;
  transaction: PublicKey;
  rentPayer: PublicKey;
  actions: SettingsAction[];
  creatorKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  memo?: string | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createCreateSettingsTransactionV2Instruction(
        {
          settings,
          transaction,
          rentPayer,
          program: programId,
        },
        {
          args: {
            actions,
            creatorKey,
            clientDataParams,
            memo,
          },
        },
        programId
      ),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
