import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function executeSettingsTransactionV2({
  connection,
  feePayer,
  settingsPda,
  transactionIndex,
  signer,
  rentPayer,
  spendingLimits,
  policies,
  extraVerificationData,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  transactionIndex: bigint;
  signer: Signer;
  rentPayer: Signer;
  spendingLimits?: PublicKey[];
  policies?: PublicKey[];
  extraVerificationData?: Uint8Array | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.executeSettingsTransactionV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    transactionIndex,
    signer: signer.publicKey,
    rentPayer: rentPayer.publicKey,
    spendingLimits,
    policies,
    extraVerificationData,
    programId,
  });

  tx.sign([feePayer, signer, rentPayer, ...(signers ?? [])]);

  try {
    return await connection.sendRawTransaction(tx.serialize(), sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
