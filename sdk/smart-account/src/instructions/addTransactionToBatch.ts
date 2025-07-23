import {
  AddressLookupTableAccount,
  PublicKey,
  TransactionMessage,
} from "@solana/web3.js";
import { createAddTransactionToBatchInstruction, PROGRAM_ID } from "../generated";
import {
  getBatchTransactionPda,
  getProposalPda,
  getTransactionPda,
  getSmartAccountPda,
} from "../pda";
import { transactionMessageToMultisigTransactionMessageBytes } from "../utils";

export function addTransactionToBatch({
  accountIndex,
  settingsPda,
  signer,
  rentPayer,
  batchIndex,
  transactionIndex,
  ephemeralSigners,
  transactionMessage,
  addressLookupTableAccounts,
  programId = PROGRAM_ID,
}: {
  accountIndex: number;
  settingsPda: PublicKey;
  /** Member of the multisig that is adding the transaction. */
  signer: PublicKey;
  /** Payer for the transaction account rent. If not provided, `member` is used. */
  rentPayer?: PublicKey;
  batchIndex: bigint;
  transactionIndex: number;
  /** Number of additional signing PDAs required by the transaction. */
  ephemeralSigners: number;
  /** Transaction message to wrap into a batch transaction. */
  transactionMessage: TransactionMessage;
  /** `AddressLookupTableAccount`s referenced in `transaction_message`. */
  addressLookupTableAccounts?: AddressLookupTableAccount[];
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

  const transactionMessageBytes =
    transactionMessageToMultisigTransactionMessageBytes({
      message: transactionMessage,
      addressLookupTableAccounts,
      smartAccountPda,
    });

  return createAddTransactionToBatchInstruction(
    {
      consensusAccount: settingsPda,
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
    },
    programId
  );
}
