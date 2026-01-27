import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createCreateTransactionFromBufferV2Instruction, PROGRAM_ID, ClientDataJsonReconstructionParams, CreateTransactionArgs } from "../generated";

export function createTransactionFromBufferV2({
  blockhash,
  feePayer,
  consensusAccount,
  transactionBuffer,
  transaction,
  rentPayer,
  createArgs,
  creatorKey,
  clientDataParams = null,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  consensusAccount: PublicKey;
  transactionBuffer: PublicKey;
  transaction: PublicKey;
  rentPayer: PublicKey;
  createArgs: CreateTransactionArgs;
  creatorKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createCreateTransactionFromBufferV2Instruction(
        {
          consensusAccount,
          transactionBuffer,
          transaction,
          rentPayer,
          program: programId,
        },
        {
          args: {
            createArgs,
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
