import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

/** Execute a settings transaction. */
export async function executeSettingsTransaction({
  connection,
  feePayer,
  settingsPda,
  transactionIndex,
  signer,
  rentPayer,
  spendingLimits,
  policies,
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
  /** In case the transaction adds or removes SpendingLimits, pass the array of their Pubkeys here. */
  spendingLimits?: PublicKey[];
  /** In case the transaction adds or removes Policies, pass the array of their Pubkeys here. */
  policies?: PublicKey[];
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.executeSettingsTransaction({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    transactionIndex,
    signer: signer.publicKey,
    rentPayer: rentPayer.publicKey,
    spendingLimits,
    policies,
    programId,
  });

  tx.sign([feePayer, signer, rentPayer, ...(signers ?? [])]);

  try {
    return await connection.sendRawTransaction(tx.serialize(), sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
