import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import { SmartAccountSigner } from "../generated";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

/** Creates a new multisig. */
export async function createSmartAccount({
  connection,
  treasury,
  createKey,
  creator,
  settings,
  settingsAuthority,
  threshold,
  signers,
  timeLock,
  rentCollector,
  memo,
  sendOptions,
  programId,
}: {
  connection: Connection;
  treasury: PublicKey;
  createKey: Signer;
  creator: Signer;
  settings: PublicKey;
  settingsAuthority: PublicKey | null;
  threshold: number;
  signers: SmartAccountSigner[];
  timeLock: number;
  rentCollector: PublicKey | null;
  memo?: string;
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createSmartAccount({
    blockhash,
    treasury,
    createKey: createKey.publicKey,
    creator: creator.publicKey,
    settings,
    settingsAuthority,
    threshold,
    signers,
    timeLock,
    rentCollector,
    memo,
    programId,
  });

  tx.sign([creator, createKey]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
