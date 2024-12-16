import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

/** Set the multisig `rent_collector`. */
export async function setRentCollectorAsAuthority({
  connection,
  feePayer,
  settingsPda,
  settingsAuthority,
  newRentCollector,
  rentPayer,
  memo,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  newRentCollector: PublicKey | null;
  rentPayer: PublicKey;
  memo?: string;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.setRentCollectorAsAuthority({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    settingsAuthority,
    newRentCollector,
    rentPayer,
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
