import * as smartAccount from "@sqds/smart-account";
import {
  Connection,
  Keypair,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";

export const sendV2Instruction = async ({
  connection,
  payer,
  instruction,
  signers = [],
}: {
  connection: Connection;
  payer: Keypair;
  instruction: TransactionInstruction;
  signers?: Keypair[];
}) => {
  const blockhash = (await connection.getLatestBlockhash()).blockhash;
  const message = new TransactionMessage({
    payerKey: payer.publicKey,
    recentBlockhash: blockhash,
    instructions: [instruction],
  }).compileToV0Message();
  const tx = new VersionedTransaction(message);
  const uniqueSigners = [payer, ...signers.filter((s) => s !== payer)];
  tx.sign(uniqueSigners);

  const signature = await connection
    .sendRawTransaction(tx.serialize())
    .catch(smartAccount.errors.translateAndThrowAnchorError);
  await connection.confirmTransaction(signature);
};
