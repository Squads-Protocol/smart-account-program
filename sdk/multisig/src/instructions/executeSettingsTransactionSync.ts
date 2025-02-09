import { AccountMeta, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  SettingsAction,
  createExecuteSettingsTransactionSyncInstruction,
  PROGRAM_ID,
} from "../generated";

export function executeSettingsTransactionSync({
  settingsPda,
  signers,
  actions,
  feePayer,
  memo,
  programId = PROGRAM_ID,
  remainingAccounts,
}: {
  settingsPda: PublicKey;
  signers: PublicKey[];
  actions: SettingsAction[];
  feePayer: PublicKey;
  memo?: string;
  programId?: PublicKey;
  remainingAccounts?: AccountMeta[];
}) {
  const ix = createExecuteSettingsTransactionSyncInstruction(
    {
      settings: settingsPda,
      rentPayer: feePayer ?? undefined,
      systemProgram: SystemProgram.programId,
      program: programId,
    },
    {
      args: {
        numSigners: signers.length,
        actions: actions,
        memo: memo ? memo : null,
      },
    },
    programId
  );
  // Append each of the signers into ix.keys
  ix.keys.push(
    ...signers.map((signer) => ({
      pubkey: signer,
      isSigner: true,
      isWritable: false,
    }))
  );
  ix.keys.push(...(remainingAccounts ?? []));
  return ix;
}
