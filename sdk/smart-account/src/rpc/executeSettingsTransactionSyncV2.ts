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
import { SettingsAction, ClientDataJsonReconstructionParams } from "../generated";

export async function executeSettingsTransactionSyncV2({
  connection,
  feePayer,
  consensusAccount,
  numNativeSigners,
  externalSignerKeyIds,
  clientDataParams,
  actions,
  memo,
  remainingAccounts,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  consensusAccount: PublicKey;
  numNativeSigners: number;
  externalSignerKeyIds: PublicKey[];
  clientDataParams?: ClientDataJsonReconstructionParams | null;
  actions: SettingsAction[];
  memo?: string | null;
  remainingAccounts?: AccountMeta[];
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.executeSettingsTransactionSyncV2({
    blockhash,
    feePayer: feePayer.publicKey,
    consensusAccount,
    numNativeSigners,
    externalSignerKeyIds,
    clientDataParams,
    actions,
    memo,
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
