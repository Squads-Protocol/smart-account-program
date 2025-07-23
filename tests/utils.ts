import { createMemoInstruction } from "@solana/spl-memo";
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SendOptions,
  SystemProgram,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import { Payload } from "@sqds/smart-account/lib/generated";
import { TransactionPayloadDetails } from "@sqds/smart-account/src/generated/types";
import assert from "assert";
import { readFileSync } from "fs";
import path from "path";

const { Permission, Permissions } = smartAccount.types;
const { Proposal } = smartAccount.accounts;

export function getTestProgramId() {
  const programKeypair = Keypair.fromSecretKey(
    Buffer.from(
      JSON.parse(
        readFileSync(
          path.join(
            __dirname,
            "../target/deploy/squads_smart_account_program-keypair.json"
          ),
          "utf-8"
        )
      )
    )
  );

  return programKeypair.publicKey;
}

export function getTestProgramConfigInitializer() {
  return Keypair.fromSecretKey(
    Buffer.from(
      JSON.parse(
        readFileSync(
          path.join(
            __dirname,
            "../test-program-config-initializer-keypair.json"
          ),
          "utf-8"
        )
      )
    )
  );
}
export function getProgramConfigInitializer() {
  return Keypair.fromSecretKey(
    Buffer.from(
      JSON.parse(
        readFileSync(
          "/Users/orion/Desktop/Squads/sqdcVVoTcKZjXU8yPUwKFbGx1Hig1rhbWJQtMRXp2E1.json",
          "utf-8"
        )
      )
    )
  );
}
export function getTestProgramConfigAuthority() {
  return Keypair.fromSecretKey(
    new Uint8Array([
      58, 1, 5, 229, 201, 214, 134, 29, 37, 52, 43, 109, 207, 214, 183, 48, 98,
      98, 141, 175, 249, 88, 126, 84, 69, 100, 223, 58, 255, 212, 102, 90, 107,
      20, 85, 127, 19, 55, 155, 38, 5, 66, 116, 148, 35, 139, 23, 147, 13, 179,
      188, 20, 37, 180, 156, 157, 85, 137, 29, 133, 29, 66, 224, 91,
    ])
  );
}

export function getTestProgramTreasury() {
  return Keypair.fromSecretKey(
    new Uint8Array([
      232, 179, 154, 90, 210, 236, 13, 219, 79, 25, 133, 75, 156, 226, 144, 171,
      193, 108, 104, 128, 11, 221, 29, 219, 139, 195, 211, 242, 231, 36, 196,
      31, 76, 110, 20, 42, 135, 60, 143, 79, 151, 67, 78, 132, 247, 97, 157, 8,
      86, 47, 10, 52, 72, 7, 88, 121, 175, 107, 108, 245, 215, 149, 242, 20,
    ])
  ).publicKey;
}
export function getTestAccountCreationAuthority() {
  return Keypair.fromSecretKey(
    Buffer.from(
      JSON.parse(
        readFileSync(
          path.join(__dirname, "../test-account-creation-authority.json"),
          "utf-8"
        )
      )
    )
  );
}
export type TestMembers = {
  almighty: Keypair;
  proposer: Keypair;
  voter: Keypair;
  executor: Keypair;
};

export async function generateFundedKeypair(connection: Connection) {
  const keypair = Keypair.generate();

  const tx = await connection.requestAirdrop(
    keypair.publicKey,
    1 * LAMPORTS_PER_SOL
  );
  await connection.confirmTransaction(tx);

  return keypair;
}

export async function fundKeypair(connection: Connection, keypair: Keypair) {
  const tx = await connection.requestAirdrop(
    keypair.publicKey,
    1 * LAMPORTS_PER_SOL
  );
  await connection.confirmTransaction(tx);
}

export async function generateSmartAccountSigners(
  connection: Connection
): Promise<TestMembers> {
  const members = {
    almighty: Keypair.generate(),
    proposer: Keypair.generate(),
    voter: Keypair.generate(),
    executor: Keypair.generate(),
  };

  // UNCOMMENT TO PRINT MEMBER PUBLIC KEYS
  // console.log("Members:");
  // for (const [name, keypair] of Object.entries(members)) {
  //   console.log(name, ":", keypair.publicKey.toBase58());
  // }

  // Airdrop 100 SOL to each member.
  await Promise.all(
    Object.values(members).map(async (member) => {
      const sig = await connection.requestAirdrop(
        member.publicKey,
        100 * LAMPORTS_PER_SOL
      );
      await connection.confirmTransaction(sig);
    })
  );

  return members;
}

export function createLocalhostConnection() {
  return new Connection("http://127.0.0.1:8899", "confirmed");
}

export const getLogs = async (
  connection: Connection,
  signature: string
): Promise<string[]> => {
  const tx = await connection.getTransaction(signature, {
    commitment: "confirmed",
  });
  return tx!.meta!.logMessages || [];
};

export async function createAutonomousMultisig({
  connection,
  accountIndex,
  members,
  threshold,
  timeLock,
  programId,
}: {
  accountIndex?: bigint;
  members: TestMembers;
  threshold: number;
  timeLock: number;
  connection: Connection;
  programId: PublicKey;
}) {
  if (!accountIndex) {
    accountIndex = await getNextAccountIndex(connection, programId);
  }
  const [settingsPda, settingsBump] = smartAccount.getSettingsPda({
    accountIndex,
    programId,
  });

  await createAutonomousSmartAccountV2({
    connection,
    accountIndex,
    members,
    threshold,
    timeLock,
    rentCollector: null,
    programId,
  });

  return [settingsPda, settingsBump] as const;
}

export async function createAutonomousSmartAccountV2({
  accountIndex,
  connection,
  members,
  threshold,
  timeLock,
  rentCollector,
  programId,
  creator,
  sendOptions,
}: {
  members: TestMembers;
  threshold: number;
  timeLock: number;
  accountIndex?: bigint;
  rentCollector: PublicKey | null;
  connection: Connection;
  programId: PublicKey;
  creator?: Keypair;
  sendOptions?: SendOptions;
}) {
  if (!creator) {
    creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);
  }

  const programConfig =
    await smartAccount.accounts.ProgramConfig.fromAccountAddress(
      connection,
      smartAccount.getProgramConfigPda({ programId })[0]
    );
  if (!accountIndex) {
    accountIndex = BigInt(programConfig.smartAccountIndex.toString()) + 1n;
  }
  const programTreasury = programConfig.treasury;
  const [settingsPda, settingsBump] = smartAccount.getSettingsPda({
    accountIndex,
    programId,
  });
  const signature = await smartAccount.rpc.createSmartAccount({
    connection,
    treasury: programTreasury,
    creator,
    settings: settingsPda,
    settingsAuthority: null,
    timeLock,
    threshold,
    signers: [
      { key: members.almighty.publicKey, permissions: Permissions.all() },
      {
        key: members.proposer.publicKey,
        permissions: Permissions.fromPermissions([Permission.Initiate]),
      },
      {
        key: members.voter.publicKey,
        permissions: Permissions.fromPermissions([Permission.Vote]),
      },
      {
        key: members.executor.publicKey,
        permissions: Permissions.fromPermissions([Permission.Execute]),
      },
    ],
    rentCollector,
    sendOptions: { skipPreflight: true },
    programId,
  });

  await connection.confirmTransaction(signature);

  return [settingsPda, settingsBump] as const;
}

export async function createControlledSmartAccount({
  connection,
  accountIndex,
  configAuthority,
  members,
  threshold,
  timeLock,
  programId,
}: {
  accountIndex: bigint;
  configAuthority: PublicKey;
  members: TestMembers;
  threshold: number;
  timeLock: number;
  connection: Connection;
  programId: PublicKey;
}) {
  const [settingsPda, settingsBump] = smartAccount.getSettingsPda({
    accountIndex,
    programId,
  });

  await createControlledMultisigV2({
    connection,
    accountIndex,
    members,
    rentCollector: null,
    threshold,
    configAuthority: configAuthority,
    timeLock,
    programId,
  });

  return [settingsPda, settingsBump] as const;
}

export async function createControlledMultisigV2({
  connection,
  accountIndex,
  configAuthority,
  members,
  threshold,
  timeLock,
  rentCollector,
  programId,
}: {
  accountIndex: bigint;
  configAuthority: PublicKey;
  members: TestMembers;
  threshold: number;
  timeLock: number;
  rentCollector: PublicKey | null;
  connection: Connection;
  programId: PublicKey;
}) {
  const creator = getTestAccountCreationAuthority();
  await fundKeypair(connection, creator);

  const [settingsPda, settingsBump] = smartAccount.getSettingsPda({
    accountIndex,
    programId,
  });
  const programConfig =
    await smartAccount.accounts.ProgramConfig.fromAccountAddress(
      connection,
      smartAccount.getProgramConfigPda({ programId })[0]
    );
  const programTreasury = programConfig.treasury;

  const signature = await smartAccount.rpc.createSmartAccount({
    connection,
    treasury: programTreasury,
    creator,
    settings: settingsPda,
    settingsAuthority: configAuthority,
    timeLock,
    threshold,
    signers: [
      { key: members.almighty.publicKey, permissions: Permissions.all() },
      {
        key: members.proposer.publicKey,
        permissions: Permissions.fromPermissions([Permission.Initiate]),
      },
      {
        key: members.voter.publicKey,
        permissions: Permissions.fromPermissions([Permission.Vote]),
      },
      {
        key: members.executor.publicKey,
        permissions: Permissions.fromPermissions([Permission.Execute]),
      },
    ],
    rentCollector,
    sendOptions: { skipPreflight: true },
    programId,
  });

  await connection.confirmTransaction(signature);

  return [settingsPda, settingsBump] as const;
}

export type MultisigWithRentReclamationAndVariousBatches = {
  settingsPda: PublicKey;
  /**
   * Index of a batch with a proposal in the Draft state.
   * The batch contains 1 transaction, which is not executed.
   * The proposal is stale.
   */
  staleDraftBatchIndex: bigint;
  /**
   * Index of a batch with a proposal in the Draft state.
   * The batch contains 1 transaction, which is not executed.
   * The proposal is stale.
   */
  staleDraftBatchNoProposalIndex: bigint;
  /**
   * Index of a batch with a proposal in the Approved state.
   * The batch contains 2 transactions, the first of which is executed, the second is not.
   * The proposal is stale.
   */
  staleApprovedBatchIndex: bigint;
  /** Index of a settings transaction that is executed, rendering the batches created before it stale. */
  executedConfigTransactionIndex: bigint;
  /**
   * Index of a batch with a proposal in the Executed state.
   * The batch contains 2 transactions, both of which are executed.
   */
  executedBatchIndex: bigint;
  /**
   * Index of a batch with a proposal in the Active state.
   * The batch contains 1 transaction, which is not executed.
   */
  activeBatchIndex: bigint;
  /**
   * Index of a batch with a proposal in the Approved state.
   * The batch contains 2 transactions, the first of which is executed, the second is not.
   */
  approvedBatchIndex: bigint;
  /**
   * Index of a batch with a proposal in the Rejected state.
   * The batch contains 1 transaction, which is not executed.
   */
  rejectedBatchIndex: bigint;
  /**
   * Index of a batch with a proposal in the Cancelled state.
   * The batch contains 1 transaction, which is not executed.
   */
  cancelledBatchIndex: bigint;
};

export async function createAutonomousMultisigWithRentReclamationAndVariousBatches({
  connection,
  members,
  threshold,
  rentCollector,
  programId,
}: {
  connection: Connection;
  members: TestMembers;
  threshold: number;
  rentCollector: PublicKey | null;
  programId: PublicKey;
}): Promise<MultisigWithRentReclamationAndVariousBatches> {
  const programConfig =
    await smartAccount.accounts.ProgramConfig.fromAccountAddress(
      connection,
      smartAccount.getProgramConfigPda({ programId })[0]
    );
  const programTreasury = programConfig.treasury;
  const accountIndex = BigInt(programConfig.smartAccountIndex.toString());
  const nextAccountIndex = accountIndex + 1n;

  const creator = getTestAccountCreationAuthority();
  await fundKeypair(connection, creator);

  const [settingsPda, settingsBump] = smartAccount.getSettingsPda({
    accountIndex: nextAccountIndex,
    programId,
  });
  const [vaultPda] = smartAccount.getSmartAccountPda({
    settingsPda,
    accountIndex: 0,
    programId,
  });

  //region Create a smart account
  let signature = await smartAccount.rpc.createSmartAccount({
    connection,
    treasury: programTreasury,
    creator,
    settings: settingsPda,
    settingsAuthority: null,
    timeLock: 0,
    threshold,
    signers: [
      { key: members.almighty.publicKey, permissions: Permissions.all() },
      {
        key: members.proposer.publicKey,
        permissions: Permissions.fromPermissions([Permission.Initiate]),
      },
      {
        key: members.voter.publicKey,
        permissions: Permissions.fromPermissions([Permission.Vote]),
      },
      {
        key: members.executor.publicKey,
        permissions: Permissions.fromPermissions([Permission.Execute]),
      },
    ],
    rentCollector,
    sendOptions: { skipPreflight: true },
    programId,
  });
  await connection.confirmTransaction(signature);
  //endregion

  //region Test instructions
  const testMessage1 = new TransactionMessage({
    payerKey: vaultPda,
    recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
    instructions: [createMemoInstruction("First memo instruction", [vaultPda])],
  });
  const testMessage2 = new TransactionMessage({
    payerKey: vaultPda,
    recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
    instructions: [
      createMemoInstruction("Second memo instruction", [vaultPda]),
    ],
  });
  //endregion

  const staleDraftBatchIndex = 1n;
  const staleDraftBatchNoProposalIndex = 2n;
  const staleApprovedBatchIndex = 3n;
  const executedConfigTransactionIndex = 4n;
  const executedBatchIndex = 5n;
  const activeBatchIndex = 6n;
  const approvedBatchIndex = 7n;
  const rejectedBatchIndex = 8n;
  const cancelledBatchIndex = 9n;

  //region Stale batch with proposal in Draft state
  // Create a batch (Stale and Non-Approved).
  signature = await smartAccount.rpc.createBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: staleDraftBatchIndex,
    accountIndex: 0,
    creator: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Create a draft proposal for the batch (Stale and Non-Approved).
  signature = await smartAccount.rpc.createProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: staleDraftBatchIndex,
    creator: members.proposer,
    isDraft: true,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Add a transaction to the batch (Stale and Non-Approved).
  signature = await smartAccount.rpc.addTransactionToBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: staleDraftBatchIndex,
    accountIndex: 0,
    transactionIndex: 1,
    transactionMessage: testMessage1,
    signer: members.proposer,
    ephemeralSigners: 0,
    programId,
  });
  await connection.confirmTransaction(signature);
  // This batch will become stale when the settings transaction is executed.
  //endregion

  //region Stale batch with No Proposal
  // Create a batch (Stale and Non-Approved).
  signature = await smartAccount.rpc.createBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: staleDraftBatchNoProposalIndex,
    accountIndex: 0,
    creator: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // No Proposal for this batch.

  // This batch will become stale when the settings transaction is executed.
  //endregion

  //region Stale batch with Approved proposal
  // Create a batch (Stale and Approved).
  signature = await smartAccount.rpc.createBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: staleApprovedBatchIndex,
    accountIndex: 0,
    creator: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Create a draft proposal for the batch (Stale and Approved).
  signature = await smartAccount.rpc.createProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: staleApprovedBatchIndex,
    creator: members.proposer,
    isDraft: true,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Add first transaction to the batch (Stale and Approved).
  signature = await smartAccount.rpc.addTransactionToBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: staleApprovedBatchIndex,
    accountIndex: 0,
    transactionIndex: 1,
    transactionMessage: testMessage1,
    signer: members.proposer,
    ephemeralSigners: 0,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Add second transaction to the batch (Stale and Approved).
  signature = await smartAccount.rpc.addTransactionToBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: staleApprovedBatchIndex,
    accountIndex: 0,
    transactionIndex: 2,
    transactionMessage: testMessage2,
    signer: members.proposer,
    ephemeralSigners: 0,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Activate the proposal (Stale and Approved).
  signature = await smartAccount.rpc.activateProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: staleApprovedBatchIndex,
    signer: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Approve the proposal (Stale and Approved).
  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.voter,
    settingsPda,
    transactionIndex: staleApprovedBatchIndex,
    signer: members.voter,
    programId,
  });
  await connection.confirmTransaction(signature);
  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.almighty,
    settingsPda,
    transactionIndex: staleApprovedBatchIndex,
    signer: members.almighty,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Execute the first batch transaction proposal (Stale and Approved).
  signature = await smartAccount.rpc.executeBatchTransaction({
    connection,
    feePayer: members.executor,
    settingsPda,
    batchIndex: staleApprovedBatchIndex,
    transactionIndex: 1,
    signer: members.executor,
    programId,
  });
  await connection.confirmTransaction(signature);
  // This proposal will become stale when the settings transaction is executed.
  //endregion

  //region Executed Config Transaction
  // Create a transaction (Executed).
  signature = await smartAccount.rpc.createSettingsTransaction({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: executedConfigTransactionIndex,
    creator: members.proposer.publicKey,
    actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
    programId,
  });
  await connection.confirmTransaction(signature);

  // Create a proposal for the transaction (Executed).
  signature = await smartAccount.rpc.createProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: executedConfigTransactionIndex,
    creator: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Approve the proposal by the first member.
  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.voter,
    settingsPda,
    transactionIndex: executedConfigTransactionIndex,
    signer: members.voter,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Approve the proposal by the second member.
  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.almighty,
    settingsPda,
    transactionIndex: executedConfigTransactionIndex,
    signer: members.almighty,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Execute the transaction.
  signature = await smartAccount.rpc.executeSettingsTransaction({
    connection,
    feePayer: members.almighty,
    settingsPda,
    transactionIndex: executedConfigTransactionIndex,
    signer: members.almighty,
    rentPayer: members.almighty,
    programId,
  });
  await connection.confirmTransaction(signature);
  //endregion

  //region batch with Executed proposal (all batch tx are executed)
  // Create a batch (Executed).
  signature = await smartAccount.rpc.createBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: executedBatchIndex,
    accountIndex: 0,
    creator: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Create a draft proposal for the batch (Executed).
  signature = await smartAccount.rpc.createProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: executedBatchIndex,
    creator: members.proposer,
    isDraft: true,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Add first transaction to the batch (Executed).
  signature = await smartAccount.rpc.addTransactionToBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: executedBatchIndex,
    accountIndex: 0,
    transactionIndex: 1,
    transactionMessage: testMessage1,
    signer: members.proposer,
    ephemeralSigners: 0,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Add second transaction to the batch (Executed).
  signature = await smartAccount.rpc.addTransactionToBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: executedBatchIndex,
    accountIndex: 0,
    transactionIndex: 2,
    transactionMessage: testMessage2,
    signer: members.proposer,
    ephemeralSigners: 0,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Activate the proposal (Executed).
  signature = await smartAccount.rpc.activateProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: executedBatchIndex,
    signer: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Approve the proposal (Executed).
  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.voter,
    settingsPda,
    transactionIndex: executedBatchIndex,
    signer: members.voter,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Execute the first batch transaction proposal (Executed).
  signature = await smartAccount.rpc.executeBatchTransaction({
    connection,
    feePayer: members.executor,
    settingsPda,
    batchIndex: executedBatchIndex,
    transactionIndex: 1,
    signer: members.executor,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Execute the second batch transaction proposal (Executed).
  signature = await smartAccount.rpc.executeBatchTransaction({
    connection,
    feePayer: members.executor,
    settingsPda,
    batchIndex: executedBatchIndex,
    transactionIndex: 2,
    signer: members.executor,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Make sure the proposal is executed.
  let proposalAccount = await Proposal.fromAccountAddress(
    connection,
    smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: executedBatchIndex,
      programId,
    })[0]
  );
  assert.ok(
    smartAccount.types.isProposalStatusExecuted(proposalAccount.status)
  );
  //endregion

  //region batch with Active proposal
  // Create a batch (Active).
  signature = await smartAccount.rpc.createBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: activeBatchIndex,
    accountIndex: 0,
    creator: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Create a draft proposal for the batch (Active).
  signature = await smartAccount.rpc.createProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: activeBatchIndex,
    creator: members.proposer,
    isDraft: true,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Add a transaction to the batch (Active).
  signature = await smartAccount.rpc.addTransactionToBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: activeBatchIndex,
    accountIndex: 0,
    transactionIndex: 1,
    transactionMessage: testMessage1,
    signer: members.proposer,
    ephemeralSigners: 0,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Activate the proposal (Active).
  signature = await smartAccount.rpc.activateProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: activeBatchIndex,
    signer: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Make sure the proposal is Active.
  proposalAccount = await Proposal.fromAccountAddress(
    connection,
    smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: activeBatchIndex,
      programId,
    })[0]
  );
  assert.ok(smartAccount.types.isProposalStatusActive(proposalAccount.status));
  //endregion

  //region batch with Approved proposal
  // Create a batch (Approved).
  signature = await smartAccount.rpc.createBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: approvedBatchIndex,
    accountIndex: 0,
    creator: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Create a draft proposal for the batch (Approved).
  signature = await smartAccount.rpc.createProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: approvedBatchIndex,
    creator: members.proposer,
    isDraft: true,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Add first transaction to the batch (Approved).
  signature = await smartAccount.rpc.addTransactionToBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: approvedBatchIndex,
    accountIndex: 0,
    transactionIndex: 1,
    transactionMessage: testMessage1,
    signer: members.proposer,
    ephemeralSigners: 0,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Add second transaction to the batch (Approved).
  signature = await smartAccount.rpc.addTransactionToBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: approvedBatchIndex,
    accountIndex: 0,
    transactionIndex: 2,
    transactionMessage: testMessage2,
    signer: members.proposer,
    ephemeralSigners: 0,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Activate the proposal (Approved).
  signature = await smartAccount.rpc.activateProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: approvedBatchIndex,
    signer: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Approve the proposal (Approved).
  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.voter,
    settingsPda,
    transactionIndex: approvedBatchIndex,
    signer: members.voter,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Make sure the proposal is Approved.
  proposalAccount = await Proposal.fromAccountAddress(
    connection,
    smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: approvedBatchIndex,
      programId,
    })[0]
  );
  assert.ok(
    smartAccount.types.isProposalStatusApproved(proposalAccount.status)
  );

  // Execute first batch transaction (Approved).
  signature = await smartAccount.rpc.executeBatchTransaction({
    connection,
    feePayer: members.executor,
    settingsPda,
    batchIndex: approvedBatchIndex,
    transactionIndex: 1,
    signer: members.executor,
    programId,
  });
  await connection.confirmTransaction(signature);
  //endregion

  //region batch with Rejected proposal
  // Create a batch (Rejected).
  signature = await smartAccount.rpc.createBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: rejectedBatchIndex,
    accountIndex: 0,
    creator: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Create a draft proposal for the batch (Rejected).
  signature = await smartAccount.rpc.createProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: rejectedBatchIndex,
    creator: members.proposer,
    isDraft: true,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Add a transaction to the batch (Rejected).
  signature = await smartAccount.rpc.addTransactionToBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: rejectedBatchIndex,
    accountIndex: 0,
    transactionIndex: 1,
    transactionMessage: testMessage1,
    signer: members.proposer,
    ephemeralSigners: 0,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Activate the proposal (Rejected).
  signature = await smartAccount.rpc.activateProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: rejectedBatchIndex,
    signer: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Reject the proposal (Rejected).
  signature = await smartAccount.rpc.rejectProposal({
    connection,
    feePayer: members.voter,
    settingsPda,
    transactionIndex: rejectedBatchIndex,
    signer: members.voter,
    programId,
  });
  await connection.confirmTransaction(signature);
  signature = await smartAccount.rpc.rejectProposal({
    connection,
    feePayer: members.almighty,
    settingsPda,
    transactionIndex: rejectedBatchIndex,
    signer: members.almighty,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Make sure the proposal is Rejected.
  proposalAccount = await Proposal.fromAccountAddress(
    connection,
    smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: rejectedBatchIndex,
      programId,
    })[0]
  );
  assert.ok(
    smartAccount.types.isProposalStatusRejected(proposalAccount.status)
  );
  //endregion

  //region batch with Cancelled proposal
  // Create a batch (Cancelled).
  signature = await smartAccount.rpc.createBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: cancelledBatchIndex,
    accountIndex: 0,
    creator: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Create a draft proposal for the batch (Cancelled).
  signature = await smartAccount.rpc.createProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: cancelledBatchIndex,
    creator: members.proposer,
    isDraft: true,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Add a transaction to the batch (Cancelled).
  signature = await smartAccount.rpc.addTransactionToBatch({
    connection,
    feePayer: members.proposer,
    settingsPda,
    batchIndex: cancelledBatchIndex,
    accountIndex: 0,
    transactionIndex: 1,
    transactionMessage: testMessage1,
    signer: members.proposer,
    ephemeralSigners: 0,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Activate the proposal (Cancelled).
  signature = await smartAccount.rpc.activateProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: cancelledBatchIndex,
    signer: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Approve the proposal (Cancelled).
  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.voter,
    settingsPda,
    transactionIndex: cancelledBatchIndex,
    signer: members.voter,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Cancel the proposal (Cancelled).
  signature = await smartAccount.rpc.cancelProposal({
    connection,
    feePayer: members.almighty,
    settingsPda,
    transactionIndex: cancelledBatchIndex,
    signer: members.almighty,
    programId,
  });
  await connection.confirmTransaction(signature);

  // Make sure the proposal is Cancelled.
  proposalAccount = await Proposal.fromAccountAddress(
    connection,
    smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: cancelledBatchIndex,
      programId,
    })[0]
  );
  assert.ok(
    smartAccount.types.isProposalStatusCancelled(proposalAccount.status)
  );
  //endregion

  return {
    settingsPda,
    staleDraftBatchIndex,
    staleDraftBatchNoProposalIndex,
    staleApprovedBatchIndex,
    executedConfigTransactionIndex,
    executedBatchIndex,
    activeBatchIndex,
    approvedBatchIndex,
    rejectedBatchIndex,
    cancelledBatchIndex,
  };
}

export function createTestTransferInstruction(
  authority: PublicKey,
  recipient: PublicKey,
  amount = 1000000
) {
  return SystemProgram.transfer({
    fromPubkey: authority,
    lamports: amount,
    toPubkey: recipient,
  });
}

/** Returns true if the given unix epoch is within a couple of seconds of now. */
export function isCloseToNow(
  unixEpoch: number | bigint,
  timeWindow: number = 2000
) {
  const timestamp = Number(unixEpoch) * 1000;
  return Math.abs(timestamp - Date.now()) < timeWindow;
}

/** Returns an array of numbers from min to max (inclusive) with the given step. */
export function range(min: number, max: number, step: number = 1) {
  const result = [];
  for (let i = min; i <= max; i += step) {
    result.push(i);
  }
  return result;
}

export function comparePubkeys(a: PublicKey, b: PublicKey) {
  return a.toBuffer().compare(b.toBuffer());
}

export async function processBufferInChunks(
  signer: Keypair,
  settingsPda: PublicKey,
  bufferAccount: PublicKey,
  buffer: Uint8Array,
  connection: Connection,
  programId: PublicKey,
  chunkSize: number = 700,
  startIndex: number = 0
) {
  const processChunk = async (startIndex: number) => {
    if (startIndex >= buffer.length) {
      return;
    }

    const chunk = buffer.slice(startIndex, startIndex + chunkSize);

    const ix = smartAccount.generated.createExtendTransactionBufferInstruction(
      {
        consensusAccount: settingsPda,
        transactionBuffer: bufferAccount,
        creator: signer.publicKey,
      },
      {
        args: {
          buffer: chunk,
        },
      },
      programId
    );

    const message = new TransactionMessage({
      payerKey: signer.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [ix],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);

    tx.sign([signer]);

    const signature = await connection.sendRawTransaction(tx.serialize(), {
      skipPreflight: true,
    });

    await connection.confirmTransaction(signature);

    // Move to next chunk
    await processChunk(startIndex + chunkSize);
  };

  await processChunk(startIndex);
}

export async function getNextAccountIndex(
  connection: Connection,
  programId: PublicKey
): Promise<bigint> {
  const [programConfigPda] = smartAccount.getProgramConfigPda({ programId });
  const programConfig =
    await smartAccount.accounts.ProgramConfig.fromAccountAddress(
      connection,
      programConfigPda,
      "processed"
    );
  const accountIndex = BigInt(programConfig.smartAccountIndex.toString());
  const nextAccountIndex = accountIndex + 1n;
  return nextAccountIndex;
}


/**
 * Extracts the TransactionPayloadDetails from a Payload.
 * @param transactionPayload - The Payload to extract the TransactionPayloadDetails from.
 * @returns The TransactionPayloadDetails.
 */
export function extractTransactionPayloadDetails(transactionPayload: Payload): TransactionPayloadDetails {
  if (transactionPayload.__kind === "TransactionPayload") {
    return transactionPayload.fields[0] as TransactionPayloadDetails
  } else {
    throw new Error("Invalid transaction payload")
  }
}
