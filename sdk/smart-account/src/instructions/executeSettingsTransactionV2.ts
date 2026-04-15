import { AccountMeta, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  createExecuteSettingsTransactionV2Instruction,
  PROGRAM_ID,
} from "../generated";
import { getProposalPda, getTransactionPda } from "../pda";
import { patchInstructionEvd } from "../utils";

export function executeSettingsTransactionV2({
  settingsPda,
  transactionIndex,
  signer,
  rentPayer,
  spendingLimits,
  policies,
  extraVerificationData,
  isExternalSigner,
  programId = PROGRAM_ID,
}: {
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: PublicKey;
  rentPayer?: PublicKey;
  spendingLimits?: PublicKey[];
  policies?: PublicKey[];
  extraVerificationData?: Uint8Array | null;
  isExternalSigner?: boolean;
  programId?: PublicKey;
}) {
  const [proposalPda] = getProposalPda({
    settingsPda,
    transactionIndex,
    programId,
  });
  const [transactionPda] = getTransactionPda({
    settingsPda,
    transactionIndex: transactionIndex,
    programId,
  });

  const remainingAccounts: AccountMeta[] = [];
  if (spendingLimits) {
    remainingAccounts.push(...spendingLimits.map((spendingLimit) => ({
      pubkey: spendingLimit,
      isWritable: true,
      isSigner: false,
    })));
  }
  if (policies) {
    remainingAccounts.push(...policies.map((policy) => ({
      pubkey: policy,
      isWritable: true,
      isSigner: false,
    })));
  }
  const ix = createExecuteSettingsTransactionV2Instruction(
    {
      settings: settingsPda,
      signer: signer,
      proposal: proposalPda,
      transaction: transactionPda,
      rentPayer: rentPayer ?? signer,
      systemProgram: SystemProgram.programId,
      anchorRemainingAccounts: remainingAccounts,
      program: programId,
    },
    { extraVerificationData: null },
    programId
  );
  if (extraVerificationData && extraVerificationData.length > 0) {
    patchInstructionEvd(ix, extraVerificationData);
  } else if (!isExternalSigner) {
    const signerMeta = ix.keys.find((k) => k.pubkey.equals(signer));
    if (signerMeta) signerMeta.isSigner = true;
  }
  return ix;
}
