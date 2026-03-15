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
import assert from "assert";
import { createSignerObject } from "./signers";
import {
  getTestAccountCreationAuthority,
  getNextAccountIndex,
} from "./connection";

const { Permission, Permissions } = smartAccount.types;
const { Proposal } = smartAccount.accounts;

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
      createSignerObject(members.almighty.publicKey, Permissions.all()),
      createSignerObject(members.proposer.publicKey, Permissions.fromPermissions([Permission.Initiate])),
      createSignerObject(members.voter.publicKey, Permissions.fromPermissions([Permission.Vote])),
      createSignerObject(members.executor.publicKey, Permissions.fromPermissions([Permission.Execute])),
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
      createSignerObject(members.almighty.publicKey, Permissions.all()),
      createSignerObject(members.proposer.publicKey, Permissions.fromPermissions([Permission.Initiate])),
      createSignerObject(members.voter.publicKey, Permissions.fromPermissions([Permission.Vote])),
      createSignerObject(members.executor.publicKey, Permissions.fromPermissions([Permission.Execute])),
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
  staleDraftBatchIndex: bigint;
  staleDraftBatchNoProposalIndex: bigint;
  staleApprovedBatchIndex: bigint;
  executedConfigTransactionIndex: bigint;
  executedBatchIndex: bigint;
  activeBatchIndex: bigint;
  approvedBatchIndex: bigint;
  rejectedBatchIndex: bigint;
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
      createSignerObject(members.almighty.publicKey, Permissions.all()),
      createSignerObject(members.proposer.publicKey, Permissions.fromPermissions([Permission.Initiate])),
      createSignerObject(members.voter.publicKey, Permissions.fromPermissions([Permission.Vote])),
      createSignerObject(members.executor.publicKey, Permissions.fromPermissions([Permission.Execute])),
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
  //endregion

  //region Stale batch with No Proposal
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
  //endregion

  //region Stale batch with Approved proposal
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

  signature = await smartAccount.rpc.activateProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: staleApprovedBatchIndex,
    signer: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

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
  //endregion

  //region Executed Config Transaction
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

  signature = await smartAccount.rpc.createProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: executedConfigTransactionIndex,
    creator: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.voter,
    settingsPda,
    transactionIndex: executedConfigTransactionIndex,
    signer: members.voter,
    programId,
  });
  await connection.confirmTransaction(signature);

  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.almighty,
    settingsPda,
    transactionIndex: executedConfigTransactionIndex,
    signer: members.almighty,
    programId,
  });
  await connection.confirmTransaction(signature);

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

  signature = await smartAccount.rpc.activateProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: executedBatchIndex,
    signer: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.voter,
    settingsPda,
    transactionIndex: executedBatchIndex,
    signer: members.voter,
    programId,
  });
  await connection.confirmTransaction(signature);

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

  signature = await smartAccount.rpc.activateProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: activeBatchIndex,
    signer: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

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

  signature = await smartAccount.rpc.activateProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: approvedBatchIndex,
    signer: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.voter,
    settingsPda,
    transactionIndex: approvedBatchIndex,
    signer: members.voter,
    programId,
  });
  await connection.confirmTransaction(signature);

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

  signature = await smartAccount.rpc.activateProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: rejectedBatchIndex,
    signer: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

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

  signature = await smartAccount.rpc.activateProposal({
    connection,
    feePayer: members.proposer,
    settingsPda,
    transactionIndex: cancelledBatchIndex,
    signer: members.proposer,
    programId,
  });
  await connection.confirmTransaction(signature);

  signature = await smartAccount.rpc.approveProposal({
    connection,
    feePayer: members.voter,
    settingsPda,
    transactionIndex: cancelledBatchIndex,
    signer: members.voter,
    programId,
  });
  await connection.confirmTransaction(signature);

  signature = await smartAccount.rpc.cancelProposal({
    connection,
    feePayer: members.almighty,
    settingsPda,
    transactionIndex: cancelledBatchIndex,
    signer: members.almighty,
    programId,
  });
  await connection.confirmTransaction(signature);

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

export async function createMintAndTransferTo(
  connection: Connection,
  payer: Keypair,
  recipient: PublicKey,
  amount: number
): Promise<[PublicKey, number]> {
  const {
    createMint,
    getOrCreateAssociatedTokenAccount,
    getAssociatedTokenAddressSync,
    TOKEN_PROGRAM_ID,
  } = await import("@solana/spl-token");

  let mintDecimals = 9;
  const mint = await createMint(
    connection,
    payer,
    payer.publicKey,
    null,
    mintDecimals,
    undefined,
    undefined,
    TOKEN_PROGRAM_ID
  );

  const associatedTokenAccount = getAssociatedTokenAddressSync(
    mint,
    recipient,
    true,
    TOKEN_PROGRAM_ID
  );

  await getOrCreateAssociatedTokenAccount(
    connection,
    payer,
    mint,
    recipient,
    true
  );

  const { createMintToInstruction } = await import("@solana/spl-token");
  const mintToIx = createMintToInstruction(
    mint,
    associatedTokenAccount,
    payer.publicKey,
    amount,
    [],
    TOKEN_PROGRAM_ID
  );

  const message = new TransactionMessage({
    payerKey: payer.publicKey,
    recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
    instructions: [mintToIx],
  }).compileToV0Message();

  const transaction = new VersionedTransaction(message);
  transaction.sign([payer]);

  const sig = await connection.sendRawTransaction(transaction.serialize(), {
    skipPreflight: true,
  });
  await connection.confirmTransaction(sig);

  return [mint, mintDecimals];
}
