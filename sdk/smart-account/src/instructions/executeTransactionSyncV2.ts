import { PublicKey } from "@solana/web3.js";
import {
  createExecuteTransactionSyncV2Instruction,
  PROGRAM_ID,
} from "../generated";

export function executeTransactionSyncV2({
  settingsPda,
  accountIndex,
  numSigners,
  instructions,
  instruction_accounts,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  accountIndex: number;
  numSigners: number;
  instructions: Uint8Array;
  instruction_accounts: {
    pubkey: PublicKey;
    isWritable: boolean;
    isSigner: boolean;
  }[];
  programId?: PublicKey;
}) {
  const ix = createExecuteTransactionSyncV2Instruction(
    {
      consensusAccount: settingsPda,
      program: programId,
      anchorRemainingAccounts: instruction_accounts,
    },
    {
      args: {
        accountIndex,
        numSigners,
        payload: {
          __kind: "Transaction",
          fields: [instructions],
        },
      },
    },
    programId
  );
  return ix;
}
