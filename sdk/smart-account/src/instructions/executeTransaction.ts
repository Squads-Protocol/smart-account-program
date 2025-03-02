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

  const [smartAccountPda] = getSmartAccountPda({
    settingsPda,
    accountIndex: transactionAccount.accountIndex,
    programId,
  });

  const { accountMetas, lookupTableAccounts } =
    await accountsForTransactionExecute({
      connection,
      message: transactionAccount.message,
      ephemeralSignerBumps: [...transactionAccount.ephemeralSignerBumps],
      smartAccountPda,
      transactionPda,
      programId,
    });

  return {
    instruction: createExecuteTransactionInstruction(
      {
        settings: settingsPda,
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
