import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function useSpendingLimit({
  connection,
  feePayer,
  signer,
  settingsPda,
  spendingLimit,
  mint,
  accountIndex,
  amount,
  decimals,
  destination,
  tokenProgram,
  memo,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  signer: Signer;
  settingsPda: PublicKey;
  spendingLimit: PublicKey;
  /** Provide if `spendingLimit` is for an SPL token, omit if it's for SOL. */
  mint?: PublicKey;
  accountIndex: number;
  amount: number;
  decimals: number;
  destination: PublicKey;
  tokenProgram?: PublicKey;
  memo?: string;
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.useSpendingLimit({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    signer: signer.publicKey,
    spendingLimit,
    mint,
    accountIndex,
    amount,
    decimals,
    destination,
    tokenProgram,
    memo,
    programId,
  });

  tx.sign([feePayer, signer]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
