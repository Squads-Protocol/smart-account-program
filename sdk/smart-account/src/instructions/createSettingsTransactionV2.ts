import { AccountMeta, PublicKey } from "@solana/web3.js";
import {
  SettingsAction,
  createCreateSettingsTransactionV2Instruction,
  PROGRAM_ID,
} from "../generated";
import { getTransactionPda } from "../pda";
import { patchInstructionEvd } from "../utils";

export function createSettingsTransactionV2({
  settingsPda,
  transactionIndex,
  creator,
  rentPayer,
  actions,
  memo,
  extraVerificationData,
  isExternalSigner,
  remainingAccounts,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  creator: PublicKey;
  rentPayer?: PublicKey;
  transactionIndex: bigint;
  actions: SettingsAction[];
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  isExternalSigner?: boolean;
  remainingAccounts?: AccountMeta[];
  programId?: PublicKey;
}) {
  const [transactionPda] = getTransactionPda({
    settingsPda,
    transactionIndex: transactionIndex,
    programId,
  });

  const ix = createCreateSettingsTransactionV2Instruction(
    {
      settings: settingsPda,
      transaction: transactionPda,
      creator,
      rentPayer: rentPayer ?? creator,
      anchorRemainingAccounts: remainingAccounts,
      program: programId,
    },
    { args: { actions, memo: memo ?? null }, extraVerificationData: null },
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
