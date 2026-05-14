import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by required signers and `feePayer` before sending it.
 */
export function executeTransactionSyncV2({
  blockhash,
  feePayer,
  settingsPda,
  accountIndex,
  numSigners,
  instructions: instructionBytes,
  instruction_accounts,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  accountIndex: number;
  numSigners: number;
  instructions: Uint8Array;
  instruction_accounts: {
    pubkey: PublicKey;
    isWritable: boolean;
    isSigner: boolean;
  }[];
  programId?: PublicKey;
}): VersionedTransaction {
  const instruction = instructions.executeTransactionSyncV2({
    settingsPda,
    accountIndex,
    numSigners,
    instructions: instructionBytes,
    instruction_accounts,
    programId,
  });

  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [instruction],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}