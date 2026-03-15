import {
  AccountMeta,
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import { translateAndThrowAnchorError } from "../errors";
import { SettingsAction } from "../generated";
import * as transactions from "../transactions";

export async function executeSettingsTransactionSyncV2({
  connection,
  feePayer,
  settingsPda,
  actions,
  memo,
  extraVerificationData,
  signers,
  remainingAccounts,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  actions: SettingsAction[];
  signers: Signer[];
  remainingAccounts?: AccountMeta[];
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.executeSettingsTransactionSyncV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    signers: signers.map((signer) => signer.publicKey),
    settingsActions: actions,
    memo,
    extraVerificationData,
    remainingAccounts,
    programId,
  });

  tx.sign([feePayer, ...signers]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
