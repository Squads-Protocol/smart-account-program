import {
  AddressLookupTableAccount,
  PublicKey,
  TransactionMessage,
} from "@solana/web3.js";
import { createAddTransactionToBatchV2Instruction, PROGRAM_ID } from "../generated";
import {
  getBatchTransactionPda,
  getProposalPda,
  getTransactionPda,
  getSmartAccountPda,
} from "../pda";
import { transactionMessageToMultisigTransactionMessageBytes, patchInstructionEvd } from "../utils";

export function addTransactionToBatchV2({
  accountIndex,
  settingsPda,
  signer,
  rentPayer,
  batchIndex,
  transactionIndex,
  ephemeralSigners,
  transactionMessage,
  addressLookupTableAccounts,
  extraVerificationData,
  isExternalSigner,
  programId = PROGRAM_ID,
}: {
  accountIndex: number;
  settingsPda: PublicKey;
  signer: PublicKey;
  rentPayer?: PublicKey;
  batchIndex: bigint;
  transactionIndex: number;
  ephemeralSigners: number;
  transactionMessage: TransactionMessage;
  addressLookupTableAccounts?: AddressLookupTableAccount[];
  extraVerificationData?: Uint8Array | null;
  isExternalSigner?: boolean;
  programId?: PublicKey;
}) {
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
  const [smartAccountPda] = getSmartAccountPda({
    settingsPda,
    accountIndex,
    programId,
  });

  const { transactionMessageBytes, compiledMessage } =
    transactionMessageToMultisigTransactionMessageBytes({
      message: transactionMessage,
      addressLookupTableAccounts,
      smartAccountPda,
    });

  const ix = createAddTransactionToBatchV2Instruction(
    {
      settings: settingsPda,
      signer,
      proposal: proposalPda,
      rentPayer: rentPayer ?? signer,
      batch: batchPda,
      transaction: batchTransactionPda,
    },
    {
      args: {
        ephemeralSigners,
        transactionMessage: transactionMessageBytes,
      },
      extraVerificationData: null,
    },
    programId
  );
  const settingsMeta = ix.keys.find((k) => k.pubkey.equals(settingsPda));
  if (settingsMeta) settingsMeta.isWritable = true;
  if (extraVerificationData && extraVerificationData.length > 0) {
    patchInstructionEvd(ix, extraVerificationData);
  } else if (!isExternalSigner) {
    const signerMeta = ix.keys.find((k) => k.pubkey.equals(signer));
    if (signerMeta) signerMeta.isSigner = true;
  }
  return ix;
}
