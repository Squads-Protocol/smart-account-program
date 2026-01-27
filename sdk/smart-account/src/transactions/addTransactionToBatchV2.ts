import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createAddTransactionToBatchV2Instruction, PROGRAM_ID, ClientDataJsonReconstructionParams } from "../generated";

export function addTransactionToBatchV2({
  blockhash,
  feePayer,
  settings,
  proposal,
  batch,
  transaction,
  signer,
  rentPayer,
  ephemeralSigners,
  transactionMessage,
  signerKey,
  clientDataParams = null,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settings: PublicKey;
  proposal: PublicKey;
  batch: PublicKey;
  transaction: PublicKey;
  signer: PublicKey;
  rentPayer: PublicKey;
  ephemeralSigners: number;
  transactionMessage: Uint8Array;
  signerKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createAddTransactionToBatchV2Instruction(
        {
          settings,
          proposal,
          batch,
          transaction,
          signer,
          rentPayer,
        },
        {
          args: {
            ephemeralSigners,
            transactionMessage,
            signerKey,
            clientDataParams,
          },
        },
        programId
      ),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
