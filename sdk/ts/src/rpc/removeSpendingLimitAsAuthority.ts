import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import { Period } from "../generated";
import * as transactions from "../transactions/index";
import { translateAndThrowAnchorError } from "../errors";

/**
 * Create a new spending limit for the controlled multisig.
 */
export async function removeSpendingLimitAsAuthority({
  connection,
  feePayer,
  settingsPda,
  settingsAuthority,
  spendingLimit,
  rentCollector,
  memo,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  spendingLimit: PublicKey;
  rentCollector: PublicKey;
  memo?: string;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.removeSpendingLimitAsAuthority({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    settingsAuthority,
    spendingLimit,
    rentCollector,
    memo,
    programId,
  });

  tx.sign([feePayer, ...(signers ?? [])]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
