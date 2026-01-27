import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";
import { ClientDataJsonReconstructionParams, SettingsAction } from "../generated";

export async function createSettingsTransactionV2({
  connection,
  feePayer,
  settings,
  transaction,
  rentPayer,
  actions,
  creatorKey,
  clientDataParams,
  memo,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settings: PublicKey;
  transaction: PublicKey;
  rentPayer: Signer;
  actions: SettingsAction[];
  creatorKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  memo?: string | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.createSettingsTransactionV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settings,
    transaction,
    rentPayer: rentPayer.publicKey,
    actions,
    creatorKey,
    clientDataParams,
    memo,
    programId,
  });

  const allSigners = [feePayer];
  if (rentPayer !== feePayer) allSigners.push(rentPayer);
  if (signers) allSigners.push(...signers);

  tx.sign(allSigners);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
