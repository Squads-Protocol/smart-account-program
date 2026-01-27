import * as smartAccount from "@sqds/smart-account";
import {
  Connection,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { TestMembers } from "../../utils";

const { Policy, Settings } = smartAccount.accounts;

export const getNextPolicyTransactionIndex = async ({
  connection,
  policyPda,
}: {
  connection: Connection;
  policyPda: PublicKey;
}) => {
  const policyAccount = await Policy.fromAccountAddress(connection, policyPda);
  return BigInt(policyAccount.transactionIndex.toString()) + 1n;
};

export const createInternalFundTransferPolicy = async ({
  connection,
  programId,
  members,
  settingsPda,
  policySeed = 1,
  sourceAccountIndices = [0],
  destinationAccountIndices = [1],
  allowedMints = [PublicKey.default],
  threshold = 1,
  timeLock = 0,
}: {
  connection: Connection;
  programId: PublicKey;
  members: TestMembers;
  settingsPda: PublicKey;
  policySeed?: number;
  sourceAccountIndices?: number[];
  destinationAccountIndices?: number[];
  allowedMints?: PublicKey[];
  threshold?: number;
  timeLock?: number;
}) => {
  const policyCreationPayload: smartAccount.generated.PolicyCreationPayload = {
    __kind: "InternalFundTransfer",
    fields: [
      {
        sourceAccountIndices: new Uint8Array(sourceAccountIndices),
        destinationAccountIndices: new Uint8Array(destinationAccountIndices),
        allowedMints,
      },
    ],
  };

  const settingsAccount = await Settings.fromAccountAddress(
    connection,
    settingsPda
  );
  const transactionIndex =
    BigInt(settingsAccount.transactionIndex.toString()) + 1n;

  const requiredIndex = Math.max(
    ...sourceAccountIndices,
    ...destinationAccountIndices
  );
  for (
    let index = settingsAccount.accountUtilization;
    index < requiredIndex;
    index += 1
  ) {
    const ix = smartAccount.generated.createIncrementAccountIndexInstruction(
      {
        settings: settingsPda,
        signer: members.proposer.publicKey,
      },
      programId
    );
    const blockhash = (await connection.getLatestBlockhash()).blockhash;
    const message = new TransactionMessage({
      payerKey: members.proposer.publicKey,
      recentBlockhash: blockhash,
      instructions: [ix],
    }).compileToV0Message();
    const tx = new VersionedTransaction(message);
    tx.sign([members.proposer]);
    const signature = await connection
      .sendRawTransaction(tx.serialize())
      .catch(smartAccount.errors.translateAndThrowAnchorError);
    await connection.confirmTransaction(signature);
  }

  const [policyPda] = smartAccount.getPolicyPda({
    settingsPda,
    policySeed,
    programId,
  });

  let signature = await smartAccount.rpc.createSettingsTransaction({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex,
    creator: members.proposer.publicKey,
    actions: [
      {
        __kind: "PolicyCreate",
        seed: policySeed,
        policyCreationPayload,
        signers: [
          {
            key: members.voter.publicKey,
            permissions: { mask: 7 },
          },
        ],
        threshold,
        timeLock,
        startTimestamp: null,
        expirationArgs: null,
      },
    ],
    programId,
  });
  await connection.confirmTransaction(signature);

  signature = await smartAccount.rpc.createProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex,
    creator: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.voter,
    settingsPda,
    transactionIndex,
    signer: members.voter,
    programId,
  });
  await connection.confirmTransaction(signature);

  signature = await smartAccount.rpc.executeSettingsTransaction({
    connection,
    feePayer: members.almighty,
    settingsPda,
    transactionIndex,
    signer: members.almighty,
    rentPayer: members.almighty,
    policies: [policyPda],
    programId,
  });
  await connection.confirmTransaction(signature);

  return { policyPda, transactionIndex };
};
