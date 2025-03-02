import { createMemoInstruction } from "@solana/spl-memo";
import {
  Keypair,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import assert from "assert";
import {
  createAutonomousMultisig,
  createAutonomousSmartAccountV2,
  createAutonomousMultisigWithRentReclamationAndVariousBatches,
  createLocalhostConnection,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  MultisigWithRentReclamationAndVariousBatches,
  TestMembers,
} from "../../utils";

const { Settings, Batch } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / batch_transaction_account_close", () => {
  let members: TestMembers;
  let settingsPda: PublicKey;
  let testMultisig: MultisigWithRentReclamationAndVariousBatches;

  // Set up a smart account with some batches.
  before(async () => {
    members = await generateSmartAccountSigners(connection);

    const accountIndex = await getNextAccountIndex(connection, programId);
    settingsPda = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    })[0];
    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    // Create new autonomous smart account with rentCollector set to its default vault.
    testMultisig =
      await createAutonomousMultisigWithRentReclamationAndVariousBatches({
        connection,
        members,
        threshold: 2,
        rentCollector: vaultPda,
        programId,
      });
  });

  it("error: wrong rent collector", async () => {
    // Create a smart account with rent reclamation disabled.
    const accountIndex = await getNextAccountIndex(connection, programId);
    const settingsPda = (
      await createAutonomousSmartAccountV2({
        connection,
        members,
        threshold: 1,
        timeLock: 0,
        rentCollector: null,
        programId,
        accountIndex,
      })
    )[0];

    const vaultPda = smartAccount.getSmartAccountPda({
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
    let signature = await smartAccount.rpc.createBatch({
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
    signature = await smartAccount.rpc.createProposal({
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
    signature = await smartAccount.rpc.addTransactionToBatch({
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
    signature = await smartAccount.rpc.activateProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: batchIndex,
      signer: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Reject the proposal.
    signature = await smartAccount.rpc.rejectProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex: batchIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);
    signature = await smartAccount.rpc.rejectProposal({
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
        smartAccount.rpc.closeBatchTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionRentCollector: Keypair.generate().publicKey,
          batchIndex,
          transactionIndex: 1,
          programId,
        }),
      /InvalidRentCollector/
    );
  });

  it("error: accounts are for another smart account", async () => {
    const vaultPda = smartAccount.getSmartAccountPda({
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

    // Create another smartAccount.
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
    let signature = await smartAccount.rpc.createBatch({
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
    signature = await smartAccount.rpc.createProposal({
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
    signature = await smartAccount.rpc.addTransactionToBatch({
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
    signature = await smartAccount.rpc.activateProposal({
      connection,
      feePayer: members.proposer,
      settingsPda: otherMultisig,
      transactionIndex: batchIndex,
      signer: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Manually construct an instruction that uses proposal account from another smartAccount.
    const ix = smartAccount.generated.createCloseBatchTransactionInstruction(
      {
        settings: settingsPda,
        transactionRentCollector: members.proposer.publicKey,
        proposal: smartAccount.getProposalPda({
          settingsPda: otherMultisig,
          transactionIndex: 1n,
          programId,
        })[0],
        batch: smartAccount.getTransactionPda({
          settingsPda,
          transactionIndex: testMultisig.rejectedBatchIndex,
          programId,
        })[0],
        transaction: smartAccount.getBatchTransactionPda({
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
          .catch(smartAccount.errors.translateAndThrowAnchorError),
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
        smartAccount.rpc.closeBatchTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionRentCollector: members.proposer.publicKey,
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
        smartAccount.rpc.closeBatchTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionRentCollector: members.proposer.publicKey,
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
        smartAccount.rpc.closeBatchTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionRentCollector: members.proposer.publicKey,
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
        smartAccount.rpc.closeBatchTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionRentCollector: members.proposer.publicKey,
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

    const signature = await smartAccount.rpc.closeBatchTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionRentCollector: members.proposer.publicKey,
      batchIndex,
      // Close one and only transaction in the batch.
      transactionIndex: 1,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Make sure the account is closed.
    const transactionPda1 = smartAccount.getBatchTransactionPda({
      settingsPda,
      batchIndex,
      transactionIndex: 1,
      programId,
    })[0];
    assert.equal(await connection.getAccountInfo(transactionPda1), null);

    // Make sure batch and proposal accounts are NOT closed.
    const batchPda = smartAccount.getTransactionPda({
      settingsPda,
      transactionIndex: batchIndex,
      programId,
    })[0];
    assert.notEqual(await connection.getAccountInfo(batchPda), null);
    const proposalPda = smartAccount.getProposalPda({
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

    const batchPda = smartAccount.getTransactionPda({
      settingsPda,
      transactionIndex: batchIndex,
      programId,
    })[0];

    let batchAccount = await Batch.fromAccountAddress(connection, batchPda);
    assert.strictEqual(batchAccount.size, 2);

    let signature = await smartAccount.rpc.closeBatchTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionRentCollector: members.proposer.publicKey,
      batchIndex,
      transactionIndex: 2,
      programId,
    });
    await connection.confirmTransaction(signature);
    // Make sure the batch size is reduced.
    batchAccount = await Batch.fromAccountAddress(connection, batchPda);
    assert.strictEqual(batchAccount.size, 1);

    signature = await smartAccount.rpc.closeBatchTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionRentCollector: members.proposer.publicKey,
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

    let signature = await smartAccount.rpc.closeBatchTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionRentCollector: members.proposer.publicKey,
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

    let signature = await smartAccount.rpc.closeBatchTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionRentCollector: members.proposer.publicKey,
      batchIndex,
      transactionIndex: 1,
      programId,
    });
    await connection.confirmTransaction(signature);
  });
});
