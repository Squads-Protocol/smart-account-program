import {
    PublicKey,
    TransactionMessage,
    VersionedTransaction,
  } from "@solana/web3.js";

  import * as instructions from "../instructions/index.js";

  /**
   * Returns unsigned `VersionedTransaction` that needs to be
   * signed by `member` and `feePayer` before sending it.
   */
  export function cancelProposal({
    blockhash,
    feePayer,
    settingsPda,
    transactionIndex,
    signer,
    memo,
    programId,
  }: {
    blockhash: string;
    feePayer: PublicKey;
    settingsPda: PublicKey;
    transactionIndex: bigint;
    signer: PublicKey;
    memo?: string;
    programId?: PublicKey;
  }): VersionedTransaction {
    const message = new TransactionMessage({
      payerKey: feePayer,
      recentBlockhash: blockhash,
    instructions: [
      instructions.cancelProposal({
        signer,
        settingsPda,
        transactionIndex,
        memo,
        programId,
        }),
      ],
    }).compileToV0Message();

    return new VersionedTransaction(message);
  }
