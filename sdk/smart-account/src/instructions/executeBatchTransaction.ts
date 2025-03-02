import {
  AddressLookupTableAccount,
  Connection,
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  Batch,
  createExecuteBatchTransactionInstruction,
  PROGRAM_ID,
  BatchTransaction,
} from "../generated";
import {
  getBatchTransactionPda,
  getProposalPda,
  getTransactionPda,
  getSmartAccountPda,
} from "../pda";
import { accountsForTransactionExecute } from "../utils";

export async function executeBatchTransaction({
  connection,
  settingsPda,
  signer,
  batchIndex,
  transactionIndex,
  programId = PROGRAM_ID,
}: {
  connection: Connection;
  settingsPda: PublicKey;
  signer: PublicKey;
  batchIndex: bigint;
  transactionIndex: number;
  programId?: PublicKey;
}): Promise<{
  instruction: TransactionInstruction;
  lookupTableAccounts: AddressLookupTableAccount[];
}> {
  const [proposalPda] = getProposalPda({
    settingsPda,
    transactionIndex: batchIndex,
    programId,
  });
  const [batchPda] = getTransactionPda({
    settingsPda,
    transactionIndex: batchIndex,
    programId,
  });
  const [batchTransactionPda] = getBatchTransactionPda({
    settingsPda,
    batchIndex,
    transactionIndex,
    programId,
  });

  const batchAccount = await Batch.fromAccountAddress(connection, batchPda);
  const [smartAccountPda] = getSmartAccountPda({
    settingsPda,
    accountIndex: batchAccount.accountIndex,
    programId,
  });

  const batchTransactionAccount =
    await BatchTransaction.fromAccountAddress(connection, batchTransactionPda);

  const { accountMetas, lookupTableAccounts } =
    await accountsForTransactionExecute({
      connection,
      message: batchTransactionAccount.message,
      ephemeralSignerBumps: [...batchTransactionAccount.ephemeralSignerBumps],
      smartAccountPda,
      transactionPda: batchPda,
    });

  return {
    instruction: createExecuteBatchTransactionInstruction(
      {
        settings: settingsPda,
        signer,
        proposal: proposalPda,
        batch: batchPda,
        transaction: batchTransactionPda,
        anchorRemainingAccounts: accountMetas,
      },
      programId
    ),
    lookupTableAccounts,
  };
}
