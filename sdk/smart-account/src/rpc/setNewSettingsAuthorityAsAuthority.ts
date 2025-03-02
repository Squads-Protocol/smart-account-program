import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

/** Set the multisig `config_authority`. */
export async function setNewSettingsAuthorityAsAuthority({
  connection,
  feePayer,
  settingsPda,
  settingsAuthority,
  newSettingsAuthority,
  memo,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  settingsAuthority: PublicKey;
  newSettingsAuthority: PublicKey;
  memo?: string;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.setNewSettingsAuthorityAsAuthority({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    settingsAuthority,
    newSettingsAuthority,
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
