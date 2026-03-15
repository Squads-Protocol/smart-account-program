import {
  AddressLookupTableAccount,
  Connection,
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  Batch,
  createExecuteBatchTransactionV2Instruction,
  PROGRAM_ID,
  BatchTransaction,
} from "../generated";
import {
  getBatchTransactionPda,
  getProposalPda,
  getTransactionPda,
  getSmartAccountPda,
} from "../pda";
import { accountsForTransactionExecute, patchInstructionEvd } from "../utils";

export async function executeBatchTransactionV2({
  connection,
  settingsPda,
  signer,
  batchIndex,
  transactionIndex,
  extraVerificationData,
  isExternalSigner,
  programId = PROGRAM_ID,
}: {
  connection: Connection;
  settingsPda: PublicKey;
  signer: PublicKey;
  batchIndex: bigint;
  transactionIndex: number;
  extraVerificationData?: Uint8Array | null;
  isExternalSigner?: boolean;
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

  const instruction = createExecuteBatchTransactionV2Instruction(
    {
      settings: settingsPda,
      signer,
      proposal: proposalPda,
      batch: batchPda,
      transaction: batchTransactionPda,
      anchorRemainingAccounts: accountMetas,
    },
    { extraVerificationData: null },
    programId
  );
  const settingsMeta = instruction.keys.find((k) => k.pubkey.equals(settingsPda));
  if (settingsMeta) settingsMeta.isWritable = true;
  if (extraVerificationData && extraVerificationData.length > 0) {
    patchInstructionEvd(instruction, extraVerificationData);
  } else if (!isExternalSigner) {
    const signerMeta = instruction.keys.find((k) => k.pubkey.equals(signer));
    if (signerMeta) signerMeta.isSigner = true;
  }
  return { instruction, lookupTableAccounts };
}
