import {
  AccountMeta,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import {
  createIncrementAccountIndexV2Instruction,
  PROGRAM_ID,
  ClientDataJsonReconstructionParams,
} from "../generated";

export function incrementAccountIndexV2({
  blockhash,
  feePayer,
  settings,
  signerKey,
  clientDataParams = null,
  anchorRemainingAccounts,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settings: PublicKey;
  signerKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  anchorRemainingAccounts?: AccountMeta[];
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createIncrementAccountIndexV2Instruction(
        {
          settings,
          anchorRemainingAccounts,
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
