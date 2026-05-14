import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by the required signers and `feePayer` before sending it.
 */
export function executeTransactionSync({
  blockhash,
  feePayer,
  settingsPda,
  accountIndex,
  numSigners,
  instructions: transactionInstructions,
  instruction_accounts,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  accountIndex: number;
  numSigners: number;
  instructions: Uint8Array;
  instruction_accounts: { pubkey: PublicKey; isWritable: boolean; isSigner: boolean }[];
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.executeTransactionSync({
        settingsPda,
        accountIndex,
        numSigners,
        instructions: transactionInstructions,
        instruction_accounts,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
