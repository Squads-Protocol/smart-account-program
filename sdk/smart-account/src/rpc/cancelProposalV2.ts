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

export async function cancelProposalV2({
  connection,
  feePayer,
  consensusAccount,
  proposal,
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
  voterKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  memo?: string | null;
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.cancelProposalV2({
    blockhash,
    feePayer: feePayer.publicKey,
    consensusAccount,
    proposal,
    voterKey,
    clientDataParams,
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
