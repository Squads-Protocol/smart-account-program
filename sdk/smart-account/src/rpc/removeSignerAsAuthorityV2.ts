import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

export async function removeSignerAsAuthorityV2({
  connection,
  feePayer,
  settings,
  settingsAuthority,
  rentPayer,
  key,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settings: PublicKey;
  settingsAuthority: Signer;
  rentPayer?: Signer;
  key: PublicKey;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.removeSignerAsAuthorityV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settings,
    settingsAuthority: settingsAuthority.publicKey,
    rentPayer: rentPayer?.publicKey,
    key,
    programId,
  });

  const allSigners = [feePayer];
  if (settingsAuthority !== feePayer) allSigners.push(settingsAuthority);
  if (rentPayer && rentPayer !== feePayer && rentPayer !== settingsAuthority) {
    allSigners.push(rentPayer);
  }
  if (signers) allSigners.push(...signers);

  tx.sign(allSigners);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
