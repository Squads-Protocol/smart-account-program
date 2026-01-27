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

export async function executeSettingsTransactionV2({
  connection,
  feePayer,
  settings,
  proposal,
  transaction,
  executorKey,
  clientDataParams,
  remainingAccounts,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settings: PublicKey;
  proposal: PublicKey;
  transaction: PublicKey;
  executorKey: PublicKey;
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  remainingAccounts?: AccountMeta[];
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.executeSettingsTransactionV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settings,
    proposal,
    transaction,
    executorKey,
    clientDataParams,
    remainingAccounts,
    programId,
  });

  tx.sign([feePayer, ...(signers ?? [])]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
