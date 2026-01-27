import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import {
  createActivateProposalV2Instruction,
  PROGRAM_ID,
  ClientDataJsonReconstructionParams,
} from "../generated";

export function activateProposalV2({
  blockhash,
  feePayer,
  consensusAccount,
  proposal,
  activatorKey,
  clientDataParams,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  consensusAccount: PublicKey;
  proposal: PublicKey;
  activatorKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createActivateProposalV2Instruction(
        {
          consensusAccount,
          proposal,
        },
        {
          args: {
            activatorKey,
            clientDataParams: clientDataParams ?? null,
          },
        },
        programId
      ),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
