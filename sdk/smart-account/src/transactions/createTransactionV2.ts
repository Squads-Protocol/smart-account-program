import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createCreateTransactionV2Instruction, PROGRAM_ID, ClientDataJsonReconstructionParams, CreateTransactionArgs } from "../generated";

export function createTransactionV2({
  blockhash,
  feePayer,
  consensusAccount,
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
      createCreateTransactionV2Instruction(
        {
          consensusAccount,
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
