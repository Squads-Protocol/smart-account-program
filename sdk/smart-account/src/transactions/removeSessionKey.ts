import {
  PublicKey,
  SystemProgram,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions/index";
import type { ClientDataJsonReconstructionParams } from "../generated";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `parentSigner` and `feePayer` before sending it.
 */
export function removeSessionKey({
  blockhash,
  feePayer,
  settingsPda,
  parentSigner,
  rentPayer,
  parentSignerKey,
  clientDataParams,
  memo,
  anchorRemainingAccounts,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  parentSigner: PublicKey;
  rentPayer: PublicKey;
  parentSignerKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams;
  memo?: string;
  anchorRemainingAccounts?: Array<{
    pubkey: PublicKey;
    isSigner: boolean;
    isWritable: boolean;
  }>;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.removeSessionKey({
        settingsPda,
        parentSigner,
        rentPayer,
        parentSignerKey,
        clientDataParams,
        memo,
        anchorRemainingAccounts,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
