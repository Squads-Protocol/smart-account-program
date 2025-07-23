import {
  Connection,
  PublicKey,
  SendOptions,
  Signer,
  TransactionSignature,
} from "@solana/web3.js";
import * as transactions from "../transactions";
import { translateAndThrowAnchorError } from "../errors";
import { PolicyPayload } from "../generated";

/**
 * Execute a policy payload synchronously with V2 instruction.
 * All required signers must be provided.
 */
export async function executePolicyPayloadSync({
  connection,
  feePayer,
  policy,
  accountIndex,
  numSigners,
  policyPayload,
  instruction_accounts,
  signers,
  sendOptions,
  programId,
}: {
  connection: Connection;
  feePayer: Signer;
  policy: PublicKey;
  accountIndex: number;
  numSigners: number;
  policyPayload: PolicyPayload;
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

  if (signers) {
    signers.map((signer) => {
      instruction_accounts.unshift({
        pubkey: signer.publicKey,
        isWritable: false,
        isSigner: true,
      });
    });
  }
  const tx = transactions.executePolicyPayloadSync({
    blockhash,
    feePayer: feePayer.publicKey,
    policy,
    accountIndex,
    numSigners,
    policyPayload,
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
