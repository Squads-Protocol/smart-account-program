import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";

/**
 * Execute a transaction synchronously with V2 instruction.
 * All required signers must be provided.
 */
export async function executeTransactionSyncV2({
  connection,
  feePayer,
  settingsPda,
  accountIndex,
  numSigners,
  instructions,
  instruction_accounts,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  settingsPda: PublicKey;
  accountIndex: number;
  numSigners: number;
  instructions: Uint8Array;
  instruction_accounts: {
    pubkey: PublicKey;
    isWritable: boolean;
    isSigner: boolean;
  }[];
  signers?: Signer[];
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.executeTransactionSyncV2({
    blockhash,
    feePayer: feePayer.publicKey,
    settingsPda,
    accountIndex,
    numSigners,
    instructions,
    instruction_accounts,
    programId,
  });

  tx.sign([feePayer, ...(signers ?? [])]);

  try {
    return await connection.sendRawTransaction(tx.serialize(), sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}