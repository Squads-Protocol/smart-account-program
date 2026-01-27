import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createExtendTransactionBufferV2Instruction, PROGRAM_ID, ClientDataJsonReconstructionParams } from "../generated";

export function extendTransactionBufferV2({
  blockhash,
  feePayer,
  consensusAccount,
  transactionBuffer,
  buffer,
  creatorKey,
  clientDataParams = null,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  consensusAccount: PublicKey;
  transactionBuffer: PublicKey;
  buffer: Uint8Array;
  creatorKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createExtendTransactionBufferV2Instruction(
        {
          consensusAccount,
          transactionBuffer,
        },
        {
          args: {
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
