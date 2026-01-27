import {
  PublicKey,
  SystemProgram,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { Permissions } from "../generated";
import * as instructions from "../instructions/index";

/**
 * Returns unsigned `VersionedTransaction` that needs to be
 * signed by `settingsAuthority` and `feePayer` before sending it.
 */
export function addSignerAsAuthorityV2({
  blockhash,
  feePayer,
  settingsPda,
  settingsAuthority,
  rentPayer,
  signerType,
  key,
  permissions,
  signerData,
  memo,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  rentPayer: PublicKey;
  signerType: number;
  key: PublicKey;
  permissions: Permissions;
  signerData: Uint8Array;
  memo?: string;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.addSignerAsAuthorityV2({
        settingsPda,
        settingsAuthority,
        rentPayer,
        signerType,
        key,
        permissions,
        signerData,
        memo,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
