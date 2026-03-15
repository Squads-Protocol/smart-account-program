import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as instructions from "../instructions";

export function createSessionKey({
  blockhash,
  feePayer,
  settingsPda,
  signer,
  args,
  extraVerificationData,
  programId,
}: {
  blockhash: string;
  feePayer: PublicKey;
  settingsPda: PublicKey;
  signer: PublicKey;
  args: {
    sessionKey: PublicKey;
    sessionKeyExpiration: bigint | number;
  };
  extraVerificationData: Uint8Array | null;
  programId?: PublicKey;
}): VersionedTransaction {
  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: [
      instructions.createSessionKey({
        settingsPda,
        signer,
        args,
        extraVerificationData,
        programId,
      }),
    ],
  }).compileToV0Message();

  return new VersionedTransaction(message);
}
