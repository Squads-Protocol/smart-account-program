import { PublicKey } from "@solana/web3.js";
import {
  createExecuteTransactionSyncV2Instruction,
  PROGRAM_ID,
  PolicyPayload,
} from "../generated";

export function executePolicyPayloadSync({
  policy,
  accountIndex,
  numSigners,
  policyPayload,
  instruction_accounts,
  programId = PROGRAM_ID,
}: {
  policy: PublicKey;
  accountIndex: number;
  numSigners: number;
  policyPayload: PolicyPayload;
  instruction_accounts: {
    pubkey: PublicKey;
    isWritable: boolean;
    isSigner: boolean;
  }[];
  programId?: PublicKey;
}) {
  const ix = createExecuteTransactionSyncV2Instruction(
    {
      consensusAccount: policy,
      program: programId,
      anchorRemainingAccounts: instruction_accounts,
    },
    {
      args: {
        accountIndex,
        numSigners,
        payload: {
          __kind: "Policy",
          fields: [policyPayload],
        },
      },
    },
    programId
  );
  return ix;
}