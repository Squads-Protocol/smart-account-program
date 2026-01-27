import {
  createCreateTransactionV2Instruction,
  ClientDataJsonReconstructionParams,
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

export function createTransactionV2({
  consensusAccount,
  transactionIndex,
  creatorKey,
  rentPayer,
  accountIndex,
  ephemeralSigners,
  transactionMessage,
  addressLookupTableAccounts,
  memo,
  createArgs,
  creator,
  creatorIsSigner = true,
  anchorRemainingAccounts,
  clientDataParams = null,
  programId = PROGRAM_ID,
}: {
  consensusAccount: PublicKey;
  transactionIndex: bigint;
  creatorKey: PublicKey;
  rentPayer: PublicKey;
  accountIndex?: number;
  /** Number of additional signing PDAs required by the transaction. */
  ephemeralSigners?: number;
  /** Transaction message to wrap into a multisig transaction. */
  transactionMessage?: TransactionMessage;
  /** `AddressLookupTableAccount`s referenced in `transaction_message`. */
  addressLookupTableAccounts?: AddressLookupTableAccount[];
  memo?: string;
  createArgs?: CreateTransactionArgs;
  creator?: PublicKey;
  creatorIsSigner?: boolean;
  anchorRemainingAccounts?: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  programId?: PublicKey;
}) {
  const [transactionPda] = getTransactionPda({
    settingsPda: consensusAccount,
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
      settingsPda: consensusAccount,
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

  const remainingAccounts = anchorRemainingAccounts ??
    (creator
      ? [
          {
            pubkey: creator,
            isSigner: creatorIsSigner,
            isWritable: false,
          },
        ]
      : undefined);

  return createCreateTransactionV2Instruction(
    {
      consensusAccount,
      transaction: transactionPda,
      rentPayer,
      program: programId,
      anchorRemainingAccounts: remainingAccounts,
    },
    {
      args: {
        createArgs: resolvedArgs,
        creatorKey,
        clientDataParams,
      },
    },
    programId
  );
}
