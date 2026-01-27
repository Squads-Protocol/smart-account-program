import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createCreateBatchV2Instruction, PROGRAM_ID, ClientDataJsonReconstructionParams } from "../generated";

export function createBatchV2({
  blockhash,
  feePayer,
  settings,
  batch,
  creator,
  rentPayer,
  accountIndex,
  creatorKey,
  memo = null,
  clientDataParams = null,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settings: PublicKey;
  batch: PublicKey;
  creator: PublicKey;
  rentPayer: PublicKey;
  accountIndex: number;
  creatorKey: PublicKey;
  memo?: string | null;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createCreateBatchV2Instruction(
        {
          settings,
          batch,
          creator,
          rentPayer,
        },
        {
          args: {
            accountIndex,
            memo,
            creatorKey,
            clientDataParams,
          },
        },
        programId
      ),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
