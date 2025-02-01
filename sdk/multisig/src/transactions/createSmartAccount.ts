import {
  AccountMeta,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { SmartAccountSigner } from "../generated";
import * as instructions from "../instructions";

/**
 * Returns unsigned `VersionedTransaction` that needs to be signed by `creator` and `createKey` before sending it.
 */
export function createSmartAccount({
  blockhash,
  treasury,
  settingsAuthority,
  creator,
  settings,
  threshold,
  signers,
  timeLock,
  rentCollector,
  memo,
  programId,
  remainingAccounts,
}: {
  blockhash: string;
  treasury: PublicKey;
  creator: PublicKey;
  settings?: PublicKey;
  settingsAuthority: PublicKey | null;
  threshold: number;
  signers: SmartAccountSigner[];
  timeLock: number;
  rentCollector: PublicKey | null;
  memo?: string;
  programId?: PublicKey;
  remainingAccounts?: AccountMeta[];
}): VersionedTransaction {
  const ix = instructions.createSmartAccount({
    treasury,
    creator,
    settings,
    settingsAuthority,
    threshold,
    signers,
    timeLock,
    rentCollector,
    memo,
    programId,
    remainingAccounts,
  });

  const message = new TransactionMessage({
    payerKey: creator,
    recentBlockhash: blockhash,
    instructions: [ix],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
