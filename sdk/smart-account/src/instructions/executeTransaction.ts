import {
  AddressLookupTableAccount,
  Connection,
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  createExecuteTransactionInstruction,
  PROGRAM_ID,
  Transaction,
  TransactionPayloadDetails,
} from "../generated";
import { getProposalPda, getSmartAccountPda, getTransactionPda } from "../pda";
import { accountsForTransactionExecute } from "../utils";

export async function executeTransaction({
  connection,
  settingsPda,
  transactionIndex,
  signer,
  programId = PROGRAM_ID,
}: {
  connection: Connection;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
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

  return {
    instruction: createExecuteTransactionInstruction(
      {
        consensusAccount: settingsPda,
        signer,
        proposal: proposalPda,
        transaction: transactionPda,
        anchorRemainingAccounts: accountMetas,
      },
      programId
    ),
    lookupTableAccounts,
  };
}
