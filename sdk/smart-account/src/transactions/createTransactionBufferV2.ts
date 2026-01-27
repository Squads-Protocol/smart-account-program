import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createCreateTransactionBufferV2Instruction, PROGRAM_ID, ClientDataJsonReconstructionParams } from "../generated";

export function createTransactionBufferV2({
  blockhash,
  feePayer,
  consensusAccount,
  transactionBuffer,
  rentPayer,
  bufferIndex,
  accountIndex,
  finalBufferHash,
  finalBufferSize,
  buffer,
  creatorKey,
  clientDataParams = null,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  consensusAccount: PublicKey;
  transactionBuffer: PublicKey;
  rentPayer: PublicKey;
  bufferIndex: number;
  accountIndex: number;
  finalBufferHash: number[];
  finalBufferSize: number;
  buffer: Uint8Array;
  creatorKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createCreateTransactionBufferV2Instruction(
        {
          consensusAccount,
          transactionBuffer,
          rentPayer,
        },
        {
          args: {
            bufferIndex,
            accountIndex,
            finalBufferHash,
            finalBufferSize,
            buffer,
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
