import { createCreateTransactionV2Instruction, PROGRAM_ID } from "../generated";
import {
  AddressLookupTableAccount,
  PublicKey,
  TransactionMessage,
} from "@solana/web3.js";
import { getTransactionPda, getSmartAccountPda } from "../pda";
import { transactionMessageToMultisigTransactionMessageBytes, patchInstructionEvd } from "../utils";

export function createTransactionV2({
  settingsPda,
  transactionIndex,
  creator,
  rentPayer,
  accountIndex,
  ephemeralSigners,
  transactionMessage,
  addressLookupTableAccounts,
  memo,
  extraVerificationData,
  isExternalSigner,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  transactionIndex: bigint;
  creator: PublicKey;
  rentPayer?: PublicKey;
  accountIndex: number;
  ephemeralSigners: number;
  transactionMessage: TransactionMessage;
  addressLookupTableAccounts?: AddressLookupTableAccount[];
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  isExternalSigner?: boolean;
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

  const ix = createCreateTransactionV2Instruction(
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
      extraVerificationData: null,
    },
    programId
  );
  if (extraVerificationData && extraVerificationData.length > 0) {
    patchInstructionEvd(ix, extraVerificationData);
  } else if (!isExternalSigner) {
    const creatorMeta = ix.keys.find((k) => k.pubkey.equals(creator));
    if (creatorMeta) creatorMeta.isSigner = true;
  }
  return ix;
}
