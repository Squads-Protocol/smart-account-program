import { createMemoInstruction } from "@solana/spl-memo";
import {
  Keypair,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import assert from "assert";
import {
  createAutonomousMultisig,
  createAutonomousMultisigV2,
  createAutonomousMultisigWithRentReclamationAndVariousBatches,
  createLocalhostConnection,
  generateFundedKeypair,
  generateMultisigMembers,
  getTestProgramId,
  MultisigWithRentReclamationAndVariousBatches,
  TestMembers,
} from "../../utils";

const { Settings, Batch } = multisig.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / vault_batch_transaction_account_close", () => {
  let members: TestMembers;
  let settingsPda: PublicKey;
  let testMultisig: MultisigWithRentReclamationAndVariousBatches;

  // Set up a multisig with some batches.
  before(async () => {
    members = await generateMultisigMembers(connection);

    const createKey = Keypair.generate();
    settingsPda = multisig.getSettingsPda({
      createKey: createKey.publicKey,
      programId,
    })[0];
    const [vaultPda] = multisig.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    // Create new autonomous multisig with rentCollector set to its default vault.
    testMultisig =
      await createAutonomousMultisigWithRentReclamationAndVariousBatches({
        connection,
        createKey,
        members,
        threshold: 2,
        rentCollector: vaultPda,
        programId,
      });
  });

  it("error: rent reclamation is not enabled", async () => {
    // Create a multisig with rent reclamation disabled.
    const settingsPda = (
      await createAutonomousMultisigV2({
        connection,
        members,
        threshold: 1,
        timeLock: 0,
        rentCollector: null,
        programId,
      })
    )[0];

    const vaultPda = multisig.getSmartAccountPda({
      settingsPda: settingsPda,
      accountIndex: 0,
      programId,
    })[0];

    const testMessage = new TransactionMessage({
      payerKey: vaultPda,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [
        createMemoInstruction("First memo instruction", [vaultPda]),
      ],
    });

    // Create a batch.
    const batchIndex = 1n;
    let signature = await multisig.rpc.createBatch({
      connection,
      feePayer: members.proposer,
      settingsPda,
      batchIndex: batchIndex,
      accountIndex: 0,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a draft proposal for the batch.
    signature = await multisig.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: batchIndex,
      creator: members.proposer,
      isDraft: true,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Add a transaction to the batch.
    signature = await multisig.rpc.addTransactionToBatch({
      connection,
      feePayer: members.proposer,
      settingsPda,
      batchIndex: batchIndex,
      accountIndex: 0,
      transactionIndex: 1,
      transactionMessage: testMessage,
      signer: members.proposer,
      ephemeralSigners: 0,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Activate the proposal.
    signature = await multisig.rpc.activateProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: batchIndex,
      signer: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Reject the proposal.
    signature = await multisig.rpc.rejectProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex: batchIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);
    signature = await multisig.rpc.rejectProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex: batchIndex,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Attempt to close the accounts.
    await assert.rejects(
      () =>
        multisig.rpc.closeBatchTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          rentCollector: Keypair.generate().publicKey,
          batchIndex,
          transactionIndex: 1,
          programId,
        }),
      /RentReclamationDisabled: Rent reclamation is disabled for this smart account/
    );
  });

  it("error: invalid rent_collector", async () => {
    const batchIndex = testMultisig.rejectedBatchIndex;

    const fakeRentCollector = Keypair.generate().publicKey;

    await assert.rejects(
      () =>
        multisig.rpc.closeBatchTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          rentCollector: fakeRentCollector,
          batchIndex,
          transactionIndex: 1,
          programId,
        }),
      /Invalid rent collector address/
    );
  });

  it("error: accounts are for another multisig", async () => {
    const vaultPda = multisig.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    })[0];

    const testMessage = new TransactionMessage({
      payerKey: vaultPda,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [
        createMemoInstruction("First memo instruction", [vaultPda]),
      ],
    });

    // Create another multisig.
    const otherMultisig = (
      await createAutonomousMultisig({
        connection,
        members,
        threshold: 2,
        timeLock: 0,
        programId,
      })
    )[0];

    // Create a batch.
    const batchIndex = 1n;
    let signature = await multisig.rpc.createBatch({
      connection,
      feePayer: members.proposer,
      settingsPda: otherMultisig,
      batchIndex: batchIndex,
      accountIndex: 0,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a draft proposal for it.
    signature = await multisig.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda: otherMultisig,
      transactionIndex: batchIndex,
      creator: members.proposer,
      isDraft: true,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Add a transaction to the batch.
    signature = await multisig.rpc.addTransactionToBatch({
      connection,
      feePayer: members.proposer,
      settingsPda: otherMultisig,
      batchIndex: batchIndex,
      accountIndex: 0,
      transactionIndex: 1,
      transactionMessage: testMessage,
      signer: members.proposer,
      ephemeralSigners: 0,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Activate the proposal.
    signature = await multisig.rpc.activateProposal({
      connection,
      feePayer: members.proposer,
      settingsPda: otherMultisig,
      transactionIndex: batchIndex,
      signer: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Manually construct an instruction that uses proposal account from another multisig.
    const ix =
      multisig.generated.createCloseBatchTransactionInstruction(
        {
          settings: settingsPda,
          rentCollector: vaultPda,
          proposal: multisig.getProposalPda({
            settingsPda: otherMultisig,
            transactionIndex: 1n,
            programId,
          })[0],
          batch: multisig.getTransactionPda({
            settingsPda,
            transactionIndex: testMultisig.rejectedBatchIndex,
            programId,
          })[0],
          transaction: multisig.getBatchTransactionPda({
            settingsPda,
            batchIndex: testMultisig.rejectedBatchIndex,
            transactionIndex: 1,
            programId,
          })[0],
        },
        programId
      );

    const feePayer = await generateFundedKeypair(connection);

    const message = new TransactionMessage({
      payerKey: feePayer.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [ix],
    }).compileToV0Message();
    const tx = new VersionedTransaction(message);
    tx.sign([feePayer]);

    await assert.rejects(
      () =>
        connection
          .sendTransaction(tx)
          .catch(multisig.errors.translateAndThrowAnchorError),
      /Proposal is for another smart account/
    );
  });

  it("error: transaction is not the last one in batch", async () => {
    const batchIndex = testMultisig.executedBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    await assert.rejects(
      () =>
        multisig.rpc.closeBatchTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          rentCollector: multisigAccount.rentCollector!,
          batchIndex,
          // The first out of two transactions.
          transactionIndex: 1,
          programId,
        }),
      /TransactionNotLastInBatch: Transaction is not last in batch/
    );
  });

  it("error: invalid proposal status (Active)", async () => {
    const batchIndex = testMultisig.activeBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    await assert.rejects(
      () =>
        multisig.rpc.closeBatchTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          rentCollector: multisigAccount.rentCollector!,
          batchIndex,
          transactionIndex: 1,
          programId,
        }),
      /Invalid proposal status/
    );
  });

  it("error: invalid proposal status (Approved and non-executed transaction)", async () => {
    const batchIndex = testMultisig.approvedBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    await assert.rejects(
      () =>
        multisig.rpc.closeBatchTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          rentCollector: multisigAccount.rentCollector!,
          batchIndex,
          // Second tx is not yet executed.
          transactionIndex: 2,
          programId,
        }),
      /Invalid proposal status/
    );
  });

  it("error: invalid proposal status (Stale but Approved and non-executed)", async () => {
    const batchIndex = testMultisig.staleApprovedBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    await assert.rejects(
      () =>
        multisig.rpc.closeBatchTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          rentCollector: multisigAccount.rentCollector!,
          batchIndex,
          // Second tx is not yet executed.
          transactionIndex: 2,
          programId,
        }),
      /Invalid proposal status/
    );
  });

  it("close batch transaction for Stale batch", async () => {
    const batchIndex = testMultisig.staleDraftBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    const signature = await multisig.rpc.closeBatchTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      rentCollector: multisigAccount.rentCollector!,
      batchIndex,
      // Close one and only transaction in the batch.
      transactionIndex: 1,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Make sure the account is closed.
    const transactionPda1 = multisig.getBatchTransactionPda({
      settingsPda,
      batchIndex,
      transactionIndex: 1,
      programId,
    })[0];
    assert.equal(await connection.getAccountInfo(transactionPda1), null);

    // Make sure batch and proposal accounts are NOT closed.
    const batchPda = multisig.getTransactionPda({
      settingsPda,
      transactionIndex: batchIndex,
      programId,
    })[0];
    assert.notEqual(await connection.getAccountInfo(batchPda), null);
    const proposalPda = multisig.getProposalPda({
      settingsPda,
      transactionIndex: batchIndex,
      programId,
    })[0];
    assert.notEqual(await connection.getAccountInfo(proposalPda), null);
  });

  it("close batch transaction for Executed batch", async () => {
    const batchIndex = testMultisig.executedBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    const batchPda = multisig.getTransactionPda({
      settingsPda,
      transactionIndex: batchIndex,
      programId,
    })[0];

    let batchAccount = await Batch.fromAccountAddress(connection, batchPda);
    assert.strictEqual(batchAccount.size, 2);

    let signature = await multisig.rpc.closeBatchTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      rentCollector: multisigAccount.rentCollector!,
      batchIndex,
      transactionIndex: 2,
      programId,
    });
    await connection.confirmTransaction(signature);
    // Make sure the batch size is reduced.
    batchAccount = await Batch.fromAccountAddress(connection, batchPda);
    assert.strictEqual(batchAccount.size, 1);

    signature = await multisig.rpc.closeBatchTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      rentCollector: multisigAccount.rentCollector!,
      batchIndex,
      transactionIndex: 1,
      programId,
    });
    await connection.confirmTransaction(signature);
    // Make sure the batch size is reduced.
    batchAccount = await Batch.fromAccountAddress(connection, batchPda);
    assert.strictEqual(batchAccount.size, 0);
  });

  it("close batch transaction for Rejected batch", async () => {
    const batchIndex = testMultisig.rejectedBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    let signature = await multisig.rpc.closeBatchTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      rentCollector: multisigAccount.rentCollector!,
      batchIndex,
      transactionIndex: 1,
      programId,
    });
    await connection.confirmTransaction(signature);
  });

  it("close batch transaction for Cancelled batch", async () => {
    const batchIndex = testMultisig.cancelledBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    let signature = await multisig.rpc.closeBatchTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      rentCollector: multisigAccount.rentCollector!,
      batchIndex,
      transactionIndex: 1,
      programId,
    });
    await connection.confirmTransaction(signature);
  });
});
