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

/** Add a member/key to the multisig and reallocate space if necessary. */
export async function addSignerAsAuthority({
  connection,
  feePayer,
  settingsPda,
  settingsAuthority,
  rentPayer,
  newSigner,
  memo,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  rentPayer: Signer;
  newSigner: SmartAccountSigner;
  memo?: string;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.addSignerAsAuthority({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    settingsAuthority,
    rentPayer: rentPayer.publicKey,
    newSigner,
    memo,
    programId,
  });

  tx.sign([feePayer, rentPayer, ...(signers ?? [])]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
