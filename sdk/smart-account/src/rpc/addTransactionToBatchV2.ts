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

export async function addTransactionToBatchV2({
  connection,
  feePayer,
  settings,
  proposal,
  batch,
  transaction,
  signer,
  rentPayer,
  ephemeralSigners,
  transactionMessage,
  signerKey,
  clientDataParams,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settings: PublicKey;
  proposal: PublicKey;
  batch: PublicKey;
  transaction: PublicKey;
  signer: Signer;
  rentPayer: Signer;
  ephemeralSigners: number;
  transactionMessage: Uint8Array;
  signerKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.addTransactionToBatchV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settings,
    proposal,
    batch,
    transaction,
    signer: signer.publicKey,
    rentPayer: rentPayer.publicKey,
    ephemeralSigners,
    transactionMessage,
    signerKey,
    clientDataParams,
    programId,
  });

  const allSigners = [feePayer];
  if (signer !== feePayer) allSigners.push(signer);
  if (rentPayer !== feePayer && rentPayer !== signer) allSigners.push(rentPayer);
  if (signers) allSigners.push(...signers);

  tx.sign(allSigners);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
