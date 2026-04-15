import { createCreateTransactionInstruction, PROGRAM_ID } from "../generated";
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
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  transactionIndex: bigint;
  creator: PublicKey;
  rentPayer?: PublicKey;
  accountIndex: number;
  /** Number of additional signing PDAs required by the transaction. */
  ephemeralSigners: number;
  /** Transaction message to wrap into a multisig transaction. */
  transactionMessage: TransactionMessage;
  /** `AddressLookupTableAccount`s referenced in `transaction_message`. */
  addressLookupTableAccounts?: AddressLookupTableAccount[];
  memo?: string;
  programId?: PublicKey;
}) {
  const [smartAccountPda] = getSmartAccountPda({
    settingsPda,
    accountIndex,
    programId,
  });

  const [transactionPda] = getTransactionPda({
    settingsPda,
    transactionIndex,
    programId,
  });

  const { transactionMessageBytes, compiledMessage } =
    transactionMessageToMultisigTransactionMessageBytes({
      message: transactionMessage,
      addressLookupTableAccounts,
      smartAccountPda,
    });

  const ix = createCreateTransactionInstruction(
    {
      consensusAccount: settingsPda,
      transaction: transactionPda,
      creator,
      rentPayer: rentPayer ?? creator,
      program: programId,
    },
    {
      args: {
        __kind: "TransactionPayload",
        fields: [
          {
            accountIndex,
            ephemeralSigners,
            transactionMessage: transactionMessageBytes,
            memo: memo ?? null,
          },
        ],
      },
    },
    programId
  );
  const consensusMeta = ix.keys.find((k) => k.pubkey.equals(settingsPda));
  if (consensusMeta) consensusMeta.isWritable = true;
  const creatorMeta = ix.keys.find((k) => k.pubkey.equals(creator));
  if (creatorMeta) creatorMeta.isSigner = true;
  return ix;
}
