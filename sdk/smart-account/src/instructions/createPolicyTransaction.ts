import {
  createCreateTransactionInstruction,
  PolicyPayload,
  PROGRAM_ID,
} from "../generated";
import {
  AddressLookupTableAccount,
  PublicKey,
  TransactionMessage,
} from "@solana/web3.js";
import { getTransactionPda, getSmartAccountPda, getPolicyPda } from "../pda";
import { transactionMessageToMultisigTransactionMessageBytes } from "../utils";

export function createPolicyTransaction({
  policy,
  transactionIndex,
  creator,
  rentPayer,
  accountIndex,
  policyPayload,
  programId = PROGRAM_ID,
}: {
  policy: PublicKey;
  transactionIndex: bigint;
  creator: PublicKey;
  rentPayer?: PublicKey;
  accountIndex: number;
  policyPayload: PolicyPayload;
  programId?: PublicKey;
}) {
  const [transactionPda] = getTransactionPda({
    settingsPda: policy,
    transactionIndex,
    programId,
  });

  return createCreateTransactionInstruction(
    {
      consensusAccount: policy,
      transaction: transactionPda,
      creator,
      rentPayer: rentPayer ?? creator,
    },
    {
      args: {
        __kind: "PolicyPayload",
        payload: policyPayload,
      },
    },
    programId
  );
}
