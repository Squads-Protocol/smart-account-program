import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import { Period, SmartAccountSigner } from "../generated";
import * as transactions from "../transactions/index";
import { translateAndThrowAnchorError } from "../errors";

/**
 * Create a new spending limit for the controlled multisig.
 */
export async function addSpendingLimitAsAuthority({
  connection,
  feePayer,
  settingsPda,
  settingsAuthority,
  spendingLimit,
  rentPayer,
  seed,
  accountIndex,
  mint,
  amount,
  period,
  destinations,
  memo,
  signers,
  sendOptions,
  programId,
  expiration,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  settingsAuthority: Signer;
  spendingLimit: PublicKey;
  rentPayer: Signer;
  seed: PublicKey;
  accountIndex: number;
  mint: PublicKey;
  amount: bigint;
  period: Period;
  signers: PublicKey[];
  destinations: PublicKey[];
  memo?: string;
  sendOptions?: SendOptions;
  programId?: PublicKey;
  expiration?: number;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.addSpendingLimitAsAuthority({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    settingsAuthority: settingsAuthority.publicKey,
    spendingLimit,
    rentPayer: rentPayer.publicKey,
    seed,
    accountIndex,
    mint,
    amount,
    signers,
    period,
    destinations,
    memo,
    programId,
    expiration,
  });

  tx.sign([feePayer, rentPayer, settingsAuthority,]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
