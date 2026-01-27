import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import BN from "bn.js";
import { createCreateProposalV2Instruction, PROGRAM_ID, ClientDataJsonReconstructionParams } from "../generated";

export function createProposalV2({
  blockhash,
  feePayer,
  consensusAccount,
  proposal,
  rentPayer,
  transactionIndex,
  draft,
  proposerKey,
  clientDataParams = null,
  memo = null,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  consensusAccount: PublicKey;
  proposal: PublicKey;
  rentPayer: PublicKey;
  transactionIndex: bigint | BN | number;
  draft: boolean;
  proposerKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  memo?: string | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createCreateProposalV2Instruction(
        {
          consensusAccount,
          proposal,
          rentPayer,
          program: programId,
        },
        {
          args: {
            transactionIndex: typeof transactionIndex === 'bigint' ? new BN(transactionIndex.toString()) : transactionIndex,
            draft,
            proposerKey,
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
