import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";
import type { ClientDataJsonReconstructionParams } from "../generated";

/**
 * Add a session key to an existing V2 external signer.
 * Session keys allow external signers to delegate temporary signing authority to a native Solana key.
 */
export async function addSessionKey({
  connection,
  feePayer,
  settingsPda,
  parentSigner,
  rentPayer,
  parentSignerKey,
  sessionKey,
  expiration,
  clientDataParams,
  memo,
  anchorRemainingAccounts,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  parentSigner: Signer;
  rentPayer: Signer;
  parentSignerKey: PublicKey;
  sessionKey: PublicKey;
  expiration: bigint | number;
  clientDataParams?: ClientDataJsonReconstructionParams;
  memo?: string;
  anchorRemainingAccounts?: Array<{
    pubkey: PublicKey;
    isSigner: boolean;
    isWritable: boolean;
  }>;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.addSessionKey({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    parentSigner: parentSigner.publicKey,
    rentPayer: rentPayer.publicKey,
    parentSignerKey,
    sessionKey,
    expiration,
    clientDataParams,
    memo,
    anchorRemainingAccounts,
    programId,
  });

  tx.sign([feePayer, parentSigner, rentPayer, ...(signers ?? [])]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
