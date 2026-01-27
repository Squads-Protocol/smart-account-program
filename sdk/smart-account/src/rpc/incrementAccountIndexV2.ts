import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";
import { ClientDataJsonReconstructionParams } from "../generated";

export async function incrementAccountIndexV2({
  connection,
  feePayer,
  settings,
  signerKey,
  clientDataParams,
  anchorRemainingAccounts,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settings: PublicKey;
  signerKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  anchorRemainingAccounts?: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.incrementAccountIndexV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settings,
    signerKey,
    clientDataParams,
    anchorRemainingAccounts,
    programId,
  });

  const allSigners = [feePayer];
  if (signers) allSigners.push(...signers);

  tx.sign(allSigners);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
