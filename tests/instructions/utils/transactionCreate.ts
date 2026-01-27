import * as smartAccount from "@sqds/smart-account";
import {
  Connection,
  Keypair,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import {
  createAutonomousMultisig,
  createTestTransferInstruction,
  TestMembers,
} from "../../utils";

const { Settings } = smartAccount.accounts;

export type TransactionArgs = {
  settingsPda: PublicKey;
  creator: Keypair;
  rentPayer: Keypair;
  accountIndex: number;
  ephemeralSigners: number;
  transactionMessage: TransactionMessage;
  memo?: string | null;
};

export type CreateTransactionV2Params = {
  settingsPda: PublicKey;
  creator: Keypair;
  rentPayer: Keypair;
  accountIndex: number;
  ephemeralSigners: number;
  transactionMessageBytes: Uint8Array;
  includeCreatorAsRemaining?: boolean;
};

export type CreateTransactionV2WithArgsParams = {
  consensusAccount: PublicKey;
  creator: Keypair;
  rentPayer: Keypair;
  transactionIndex: bigint;
  createArgs: smartAccount.generated.CreateTransactionArgs;
  includeCreatorAsRemaining?: boolean;
};

export const createSettings = async ({
  connection,
  members,
  programId,
  timeLock = 0,
}: {
  connection: Connection;
  members: TestMembers;
  programId: PublicKey;
  timeLock?: number;
}) =>
  (
    await createAutonomousMultisig({
      connection,
      members,
      threshold: 1,
      timeLock,
      programId,
    })
  )[0];

export const buildTestMessage = async ({
  connection,
  settingsPda,
  accountIndex,
  programId,
}: {
  connection: Connection;
  settingsPda: PublicKey;
  accountIndex: number;
  programId: PublicKey;
}) => {
  const [smartAccountPda] = smartAccount.getSmartAccountPda({
    settingsPda,
    accountIndex,
    programId,
  });
  const ix = createTestTransferInstruction(
    smartAccountPda,
    Keypair.generate().publicKey
  );
  const message = new TransactionMessage({
    payerKey: smartAccountPda,
    recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
    instructions: [ix],
  });

  return { smartAccountPda, message };
};

export const getNextTransactionIndex = async ({
  connection,
  settingsPda,
}: {
  connection: Connection;
  settingsPda: PublicKey;
}) => {
  const settingsAccount = await Settings.fromAccountAddress(
    connection,
    settingsPda
  );
  return BigInt(settingsAccount.transactionIndex.toString()) + 1n;
};

export const createTransactionV1 = async ({
  connection,
  programId,
  settingsPda,
  creator,
  rentPayer,
  accountIndex,
  ephemeralSigners,
  transactionMessage,
  memo = null,
}: TransactionArgs & {
  connection: Connection;
  programId: PublicKey;
}) => {
  const transactionIndex = await getNextTransactionIndex({
    connection,
    settingsPda,
  });
  const signature = await smartAccount.rpc.createTransaction({
    connection,
    feePayer: creator,
    settingsPda,
    transactionIndex,
    creator: creator.publicKey,
    rentPayer: rentPayer.publicKey,
    accountIndex,
    ephemeralSigners,
    transactionMessage,
    memo: memo ?? undefined,
    signers: rentPayer === creator ? undefined : [rentPayer],
    programId,
  });
  await connection.confirmTransaction(signature);

  const [transactionPda] = smartAccount.getTransactionPda({
    settingsPda,
    transactionIndex,
    programId,
  });

  return { transactionIndex, transactionPda };
};

export const sendCreateTransactionV2WithArgs = async ({
  connection,
  programId,
  consensusAccount,
  creator,
  rentPayer,
  transactionIndex,
  createArgs,
  includeCreatorAsRemaining = true,
}: CreateTransactionV2WithArgsParams & {
  connection: Connection;
  programId: PublicKey;
}) => {
  const [transactionPda] = smartAccount.getTransactionPda({
    settingsPda: consensusAccount,
    transactionIndex,
    programId,
  });

  const ix = smartAccount.generated.createCreateTransactionV2Instruction(
    {
      consensusAccount,
      transaction: transactionPda,
      rentPayer: rentPayer.publicKey,
      program: programId,
      anchorRemainingAccounts: includeCreatorAsRemaining
        ? [
            {
              pubkey: creator.publicKey,
              isSigner: true,
              isWritable: false,
            },
          ]
        : [],
    },
    {
      args: {
        createArgs,
        creatorKey: creator.publicKey,
        clientDataParams: null,
      },
    },
    programId
  );

  const blockhash = (await connection.getLatestBlockhash()).blockhash;
  const message = new TransactionMessage({
    payerKey: creator.publicKey,
    recentBlockhash: blockhash,
    instructions: [ix],
  }).compileToV0Message();
  const tx = new VersionedTransaction(message);

  const signers = [creator];
  if (rentPayer.publicKey.toBase58() !== creator.publicKey.toBase58()) {
    signers.push(rentPayer);
  }
  tx.sign(signers);

  const signature = await connection
    .sendRawTransaction(tx.serialize())
    .catch(smartAccount.errors.translateAndThrowAnchorError);
  await connection.confirmTransaction(signature);

  return { transactionIndex, transactionPda };
};

export const sendCreateTransactionV2 = async ({
  connection,
  programId,
  settingsPda,
  creator,
  rentPayer,
  accountIndex,
  ephemeralSigners,
  transactionMessageBytes,
  includeCreatorAsRemaining = true,
}: CreateTransactionV2Params & {
  connection: Connection;
  programId: PublicKey;
}) => {
  const transactionIndex = await getNextTransactionIndex({
    connection,
    settingsPda,
  });
  const createArgs = {
    __kind: "TransactionPayload",
    fields: [
      {
        accountIndex,
        ephemeralSigners,
        transactionMessage: transactionMessageBytes,
        memo: null,
      },
    ],
  } as unknown as smartAccount.generated.CreateTransactionArgs;

  return sendCreateTransactionV2WithArgs({
    connection,
    programId,
    consensusAccount: settingsPda,
    creator,
    rentPayer,
    transactionIndex,
    createArgs,
    includeCreatorAsRemaining,
  });
};

export const buildTransactionMessageBytes = async ({
  connection,
  programId,
  settingsPda,
  accountIndex,
}: {
  connection: Connection;
  programId: PublicKey;
  settingsPda: PublicKey;
  accountIndex: number;
}) => {
  const { message, smartAccountPda } = await buildTestMessage({
    connection,
    settingsPda,
    accountIndex,
    programId,
  });
  const { transactionMessageBytes } =
    smartAccount.utils.transactionMessageToMultisigTransactionMessageBytes({
      message,
      smartAccountPda,
    });
  return transactionMessageBytes;
};

export const sendV1WithArgs = async ({
  connection,
  programId,
  settingsPda,
  creator,
  rentPayer,
  args,
}: {
  connection: Connection;
  programId: PublicKey;
  settingsPda: PublicKey;
  creator: Keypair;
  rentPayer: Keypair;
  args: smartAccount.generated.CreateTransactionArgs;
}) => {
  const transactionIndex = await getNextTransactionIndex({
    connection,
    settingsPda,
  });
  const [transactionPda] = smartAccount.getTransactionPda({
    settingsPda,
    transactionIndex,
    programId,
  });

  const ix = smartAccount.generated.createCreateTransactionInstruction(
    {
      consensusAccount: settingsPda,
      transaction: transactionPda,
      creator: creator.publicKey,
      rentPayer: rentPayer.publicKey,
      program: programId,
    },
    { args },
    programId
  );

  const blockhash = (await connection.getLatestBlockhash()).blockhash;
  const message = new TransactionMessage({
    payerKey: creator.publicKey,
    recentBlockhash: blockhash,
    instructions: [ix],
  }).compileToV0Message();
  const tx = new VersionedTransaction(message);
  tx.sign([creator, rentPayer]);

  return connection
    .sendRawTransaction(tx.serialize())
    .catch(smartAccount.errors.translateAndThrowAnchorError);
};
