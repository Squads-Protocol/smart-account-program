import {
  AccountMeta,
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import { SettingsAction } from "../generated";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function createSettingsTransactionV2({
  connection,
  feePayer,
  settingsPda,
  transactionIndex,
  creator,
  rentPayer,
  actions,
  memo,
  extraVerificationData,
  signers,
  sendOptions,
  remainingAccounts,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  creator: PublicKey;
  rentPayer?: PublicKey;
  actions: SettingsAction[];
  memo?: string;
  extraVerificationData?: Uint8Array | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  remainingAccounts?: AccountMeta[];
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createSettingsTransactionV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    transactionIndex,
    creator,
    rentPayer,
    actions,
    memo,
    extraVerificationData,
    remainingAccounts,
    programId,
  });

  tx.sign([feePayer, ...(signers ?? [])]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
