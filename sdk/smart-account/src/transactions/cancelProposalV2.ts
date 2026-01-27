import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createCancelProposalV2Instruction, PROGRAM_ID, ClientDataJsonReconstructionParams } from "../generated";

export function cancelProposalV2({
  blockhash,
  feePayer,
  consensusAccount,
  proposal,
  voterKey,
  clientDataParams = null,
  memo = null,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  consensusAccount: PublicKey;
  proposal: PublicKey;
  voterKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  memo?: string | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createCancelProposalV2Instruction(
        {
          consensusAccount,
          proposal,
          program: programId,
        },
        {
          args: {
            voterKey,
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
