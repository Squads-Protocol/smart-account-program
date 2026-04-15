import {
  AddressLookupTableAccount,
  Connection,
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  createExecuteTransactionV2Instruction,
  PROGRAM_ID,
  Transaction,
  TransactionPayloadDetails,
} from "../generated";
import { getProposalPda, getSmartAccountPda, getTransactionPda } from "../pda";
import { accountsForTransactionExecute, patchInstructionEvd } from "../utils";

export async function executeTransactionV2({
  connection,
  settingsPda,
  transactionIndex,
  signer,
  extraVerificationData,
  isExternalSigner,
  programId = PROGRAM_ID,
}: {
  connection: Connection;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  extraVerificationData?: Uint8Array | null;
  isExternalSigner?: boolean;
  programId?: PublicKey;
}): Promise<{
  instruction: TransactionInstruction;
  lookupTableAccounts: AddressLookupTableAccount[];
}> {
  const [proposalPda] = getProposalPda({
    settingsPda,
    transactionIndex,
    programId,
  });
  const [transactionPda] = getTransactionPda({
    settingsPda,
    transactionIndex,
    programId,
  });
  const transactionAccount = await Transaction.fromAccountAddress(
    connection,
    transactionPda
  );
  const transactionPayload = transactionAccount.payload
  let transactionDetails: TransactionPayloadDetails
  if (transactionPayload.__kind === "TransactionPayload") {
    transactionDetails = transactionPayload.fields[0]
  } else {
    throw new Error("Invalid transaction payload")
  }

  const [smartAccountPda] = getSmartAccountPda({
    settingsPda,
    accountIndex: transactionDetails.accountIndex,
    programId,
  });

  const { accountMetas, lookupTableAccounts } =
    await accountsForTransactionExecute({
      connection,
      message: transactionDetails.message,
      ephemeralSignerBumps: [...transactionDetails.ephemeralSignerBumps],
      smartAccountPda,
      transactionPda,
      programId,
    });

  const instruction = createExecuteTransactionV2Instruction(
    {
      consensusAccount: settingsPda,
      signer,
      proposal: proposalPda,
      transaction: transactionPda,
      program: programId,
      anchorRemainingAccounts: accountMetas,
    },
    { extraVerificationData: null },
    programId
  );
  if (extraVerificationData && extraVerificationData.length > 0) {
    patchInstructionEvd(instruction, extraVerificationData);
  } else if (!isExternalSigner) {
    const signerMeta = instruction.keys.find((k) => k.pubkey.equals(signer));
    if (signerMeta) signerMeta.isSigner = true;
  }
  return { instruction, lookupTableAccounts };
}
