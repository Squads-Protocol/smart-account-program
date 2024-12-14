import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import { translateAndThrowAnchorError } from "../errors";
import { ConfigAction } from "../generated";
import * as transactions from "../transactions";

/**
 * Synchronously execute configuration changes for the multisig.
 */
export async function configTransactionSync({
  connection,
  feePayer,
  multisigPda,
  configActions,
  memo,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  multisigPda: PublicKey;
  configActions: ConfigAction[];
  signers: Signer[];
  memo?: string;
  sendOptions?: SendOptions;
  programId?: PublicKey;
}): Promise<TransactionSignature> {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;

  const tx = transactions.configTransactionSync({
    blockhash,
    feePayer: feePayer.publicKey,
    multisigPda,
    signers: signers.map(signer => signer.publicKey),
    configActions,
    memo,
    programId,
  });

  tx.sign([feePayer, ...(signers)]);

  try {
    return await connection.sendTransaction(tx, sendOptions);
  } catch (err) {
    translateAndThrowAnchorError(err);
  }
}
