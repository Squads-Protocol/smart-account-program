import {
  AccountMeta,
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";
import { ClientDataJsonReconstructionParams } from "../generated";

export async function executeBatchTransactionV2({
  connection,
  feePayer,
  settings,
  signer,
  proposal,
  batch,
  transaction,
  signerKey,
  clientDataParams,
  remainingAccounts,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settings: PublicKey;
  signer: Signer;
  proposal: PublicKey;
  batch: PublicKey;
  transaction: PublicKey;
  signerKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  remainingAccounts?: AccountMeta[];
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.executeBatchTransactionV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settings,
    signer: signer.publicKey,
    proposal,
    batch,
    transaction,
    signerKey,
    clientDataParams,
    remainingAccounts,
    programId,
  });

  const allSigners = [feePayer];
  if (signer !== feePayer) allSigners.push(signer);
  if (signers) allSigners.push(...signers);

  tx.sign(allSigners);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
