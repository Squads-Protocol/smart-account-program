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

export async function approveProposalV2({
  connection,
  feePayer,
  consensusAccount,
  proposal,
  payer,
  voterKey,
  clientDataParams,
  memo,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  consensusAccount: PublicKey;
  proposal: PublicKey;
  payer?: Signer;
  voterKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  memo?: string | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.approveProposalV2({
    blockhash,
    feePayer: feePayer.publicKey,
    consensusAccount,
    proposal,
    payer: payer?.publicKey,
    voterKey,
    clientDataParams,
    memo,
    programId,
  });

  const allSigners = [feePayer];
  if (payer && payer !== feePayer) allSigners.push(payer);
  if (signers) allSigners.push(...signers);

  tx.sign(allSigners);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
