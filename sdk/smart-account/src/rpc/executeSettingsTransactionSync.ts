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

/**
 * Synchronously execute configuration changes for the multisig.
 */
export async function executeSettingsTransactionSync({
  connection,
  feePayer,
  settingsPda,
  actions,
  memo,
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
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.executeSettingsTransactionSync({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    signers: signers.map((signer) => signer.publicKey),
    settingsActions: actions,
    memo,
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
