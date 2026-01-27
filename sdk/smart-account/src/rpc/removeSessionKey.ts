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
 * Remove a session key from an existing V2 external signer.
 * This revokes the temporary signing authority that was delegated to the session key.
 */
export async function removeSessionKey({
  connection,
  feePayer,
  settingsPda,
  parentSigner,
  rentPayer,
  parentSignerKey,
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

  const tx = transactions.removeSessionKey({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    parentSigner: parentSigner.publicKey,
    rentPayer: rentPayer.publicKey,
    parentSignerKey,
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
