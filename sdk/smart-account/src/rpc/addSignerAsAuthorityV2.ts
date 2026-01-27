import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import { Permissions } from "../generated";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

/** Add a signer with external signer support and reallocate space if necessary. */
export async function addSignerAsAuthorityV2({
  connection,
  feePayer,
  settingsPda,
  settingsAuthority,
  rentPayer,
  signerType,
  key,
  permissions,
  signerData,
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
  signerType: number;
  key: PublicKey;
  permissions: Permissions;
  signerData: Uint8Array;
  memo?: string;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.addSignerAsAuthorityV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    settingsAuthority,
    rentPayer: rentPayer.publicKey,
    signerType,
    key,
    permissions,
    signerData,
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
