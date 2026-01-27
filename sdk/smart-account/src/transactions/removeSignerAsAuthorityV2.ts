import {
  PublicKey,
  SystemProgram,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createRemoveSignerAsAuthorityV2Instruction, PROGRAM_ID } from "../generated";

export function removeSignerAsAuthorityV2({
  blockhash,
  feePayer,
  settings,
  settingsAuthority,
  rentPayer,
  key,
  memo = null,
  programId = PROGRAM_ID,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settings: PublicKey;
  settingsAuthority: PublicKey;
  rentPayer?: PublicKey;
  key: PublicKey;
  memo?: string | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      createRemoveSignerAsAuthorityV2Instruction(
        {
          settings,
          settingsAuthority,
          rentPayer,
          systemProgram: SystemProgram.programId,
          program: programId,
        },
        {
          args: {
            key,
            memo,
          },
        },
        programId
      ),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
