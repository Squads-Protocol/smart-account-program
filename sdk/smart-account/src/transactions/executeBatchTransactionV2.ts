import {
  AccountMeta,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createExecuteBatchTransactionV2Instruction, PROGRAM_ID, ClientDataJsonReconstructionParams } from "../generated";

export function executeBatchTransactionV2({
  blockhash,
  feePayer,
  settings,
  signer,
  proposal,
  batch,
  transaction,
  signerKey,
  clientDataParams = null,
  remainingAccounts,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settings: PublicKey;
  signer: PublicKey;
  proposal: PublicKey;
  batch: PublicKey;
  transaction: PublicKey;
  signerKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  remainingAccounts?: AccountMeta[];
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createExecuteBatchTransactionV2Instruction(
        {
          settings,
          signer,
          proposal,
          batch,
          transaction,
          anchorRemainingAccounts: remainingAccounts,
        },
        {
          args: {
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
