import {
  createCreateTransactionInstruction,
  CreateTransactionArgs,
  PROGRAM_ID,
} from "../generated";
import {
  AddressLookupTableAccount,
  PublicKey,
  TransactionMessage,
} from "@solana/web3.js";
import { getTransactionPda, getSmartAccountPda } from "../pda";
import { transactionMessageToMultisigTransactionMessageBytes } from "../utils";

export function createTransaction({
  settingsPda,
  transactionIndex,
  creator,
  rentPayer,
  accountIndex,
  ephemeralSigners,
  transactionMessage,
  addressLookupTableAccounts,
  memo,
  createArgs,
  consensusAccount,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  transactionIndex: bigint;
  creator: PublicKey;
  rentPayer?: PublicKey;
  accountIndex?: number;
  /** Number of additional signing PDAs required by the transaction. */
  ephemeralSigners?: number;
  /** Transaction message to wrap into a multisig transaction. */
  transactionMessage?: TransactionMessage;
  /** `AddressLookupTableAccount`s referenced in `transaction_message`. */
  addressLookupTableAccounts?: AddressLookupTableAccount[];
  memo?: string;
  createArgs?: CreateTransactionArgs;
  consensusAccount?: PublicKey;
  programId?: PublicKey;
}) {
  const targetConsensusAccount = consensusAccount ?? settingsPda;
  const [transactionPda] = getTransactionPda({
    settingsPda: targetConsensusAccount,
    transactionIndex,
    programId,
  });

  let resolvedArgs: CreateTransactionArgs;
  if (createArgs) {
    resolvedArgs = createArgs;
  } else {
    if (
      transactionMessage == null ||
      accountIndex == null ||
      ephemeralSigners == null
    ) {
      throw new Error(
        "transactionMessage, accountIndex, and ephemeralSigners are required when createArgs is not provided"
      );
    }
    const [smartAccountPda] = getSmartAccountPda({
      settingsPda,
      accountIndex,
      programId,
    });
    const { transactionMessageBytes } =
      transactionMessageToMultisigTransactionMessageBytes({
        message: transactionMessage,
        addressLookupTableAccounts,
        smartAccountPda,
      });
    resolvedArgs = {
      __kind: "TransactionPayload",
      fields: [
        {
          accountIndex,
          ephemeralSigners,
          transactionMessage: transactionMessageBytes,
          memo: memo ?? null,
        },
      ],
    } as CreateTransactionArgs;
  }

  return createCreateTransactionInstruction(
    {
      consensusAccount: targetConsensusAccount,
      transaction: transactionPda,
      creator,
      rentPayer: rentPayer ?? creator,
      program: programId,
    },
    {
      args: resolvedArgs,
    },
    programId
  );
}
