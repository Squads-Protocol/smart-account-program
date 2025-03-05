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

const { Settings } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / batch_accounts_close", () => {
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

  it("error: rent reclamation is not enabled", async () => {
    // Create a smart account with rent reclamation disabled.
    const accountIndex = await getNextAccountIndex(connection, programId);
    const settingsPda = (
      await createAutonomousSmartAccountV2({
        connection,
        members,
        threshold: 1,
        timeLock: 0,
        accountIndex,
        programId,
        rentCollector: null,
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
        smartAccount.rpc.closeBatch({
          connection,
          feePayer: members.almighty,
          settingsPda,
          batchRentCollector: Keypair.generate().publicKey,
          batchIndex,
          programId,
        }),
      /InvalidRentCollector/
    );
  });

  it("error: invalid rent_collector", async () => {
    const batchIndex = testMultisig.rejectedBatchIndex;

    const fakeRentCollector = Keypair.generate().publicKey;

    await assert.rejects(
      () =>
        smartAccount.rpc.closeBatch({
          connection,
          feePayer: members.almighty,
          settingsPda,
          batchRentCollector: fakeRentCollector,
          batchIndex,
          programId,
        }),
      /Invalid rent collector address/
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
    const accountIndex = await getNextAccountIndex(connection, programId);
    // Create another smartAccount.
    const otherMultisig = (
      await createAutonomousMultisig({
        connection,
        members,
        threshold: 2,
        timeLock: 0,
        accountIndex,
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
    const ix = smartAccount.generated.createCloseBatchInstruction(
      {
        settings: settingsPda,
        batchRentCollector: vaultPda,
        proposalRentCollector: vaultPda,
        proposal: smartAccount.getProposalPda({
          settingsPda,
          transactionIndex: 1n,
          programId,
        })[0],
        batch: smartAccount.getTransactionPda({
          settingsPda: otherMultisig,
          transactionIndex: 1n,
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
      /Transaction is for another smart account/
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
        smartAccount.rpc.closeBatch({
          connection,
          feePayer: members.almighty,
          settingsPda,
          batchRentCollector: members.proposer.publicKey,
          batchIndex,
          programId,
        }),
      /Invalid proposal status/
    );
  });

  it("error: invalid proposal status (Approved)", async () => {
    const batchIndex = testMultisig.approvedBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    await assert.rejects(
      () =>
        smartAccount.rpc.closeBatch({
          connection,
          feePayer: members.almighty,
          settingsPda,
          batchRentCollector: members.proposer.publicKey,
          proposalRentCollector: members.proposer.publicKey,
          batchIndex,
          programId,
        }),
      /Invalid proposal status/
    );
  });

  it("error: invalid proposal status (Stale but Approved)", async () => {
    const batchIndex = testMultisig.staleApprovedBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    await assert.rejects(
      () =>
        smartAccount.rpc.closeBatch({
          connection,
          feePayer: members.almighty,
          settingsPda,
          batchRentCollector: members.proposer.publicKey,
          proposalRentCollector: members.proposer.publicKey,
          batchIndex,
          programId,
        }),
      /Invalid proposal status/
    );
  });

  it("error: batch is not empty", async () => {
    const batchIndex = testMultisig.executedBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    await assert.rejects(
      () =>
        smartAccount.rpc.closeBatch({
          connection,
          feePayer: members.almighty,
          settingsPda,
          batchRentCollector: members.proposer.publicKey,
          batchIndex,
          programId,
        }),
      /BatchNotEmpty: Batch is not empty/
    );
  });

  it("close accounts for Stale batch", async () => {
    const batchIndex = testMultisig.staleDraftBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    // First close the transaction.
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

    signature = await smartAccount.rpc.closeBatch({
      connection,
      feePayer: members.almighty,
      settingsPda,
      batchRentCollector: members.proposer.publicKey,
      proposalRentCollector: members.proposer.publicKey,
      batchIndex,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Make sure batch and proposal accounts are closed.
    const batchPda = smartAccount.getTransactionPda({
      settingsPda,
      transactionIndex: batchIndex,
      programId,
    })[0];
    assert.equal(await connection.getAccountInfo(batchPda), null);
    const proposalPda = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: batchIndex,
      programId,
    })[0];
    assert.equal(await connection.getAccountInfo(proposalPda), null);
  });

  it("close accounts for Stale batch with no Proposal", async () => {
    const batchIndex = testMultisig.staleDraftBatchNoProposalIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    const proposalPda = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: batchIndex,
      programId,
    })[0];

    // Make sure proposal account doesn't exist.
    assert.equal(await connection.getAccountInfo(proposalPda), null);

    let signature = await smartAccount.rpc.closeBatch({
      connection,
      feePayer: members.almighty,
      settingsPda,
      batchRentCollector: members.proposer.publicKey,
      proposalRentCollector: members.proposer.publicKey,
      batchIndex,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Make sure batch and proposal accounts are closed.
    const batchPda = smartAccount.getTransactionPda({
      settingsPda,
      transactionIndex: batchIndex,
      programId,
    })[0];
    assert.equal(await connection.getAccountInfo(batchPda), null);
  });

  it("close accounts for Executed batch", async () => {
    const batchIndex = testMultisig.executedBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    // First close the vault transactions.
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

    signature = await smartAccount.rpc.closeBatch({
      connection,
      feePayer: members.almighty,
      settingsPda,
      batchRentCollector: members.proposer.publicKey,
      proposalRentCollector: members.proposer.publicKey,
      batchIndex,
      programId,
    });
    await connection.confirmTransaction(signature);
  });

  it("close accounts for Rejected batch", async () => {
    const batchIndex = testMultisig.rejectedBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    // First close the vault transactions.
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

    signature = await smartAccount.rpc.closeBatch({
      connection,
      feePayer: members.almighty,
      settingsPda,
      batchRentCollector: members.proposer.publicKey,
      proposalRentCollector: members.proposer.publicKey,
      batchIndex,
      programId,
    });
    await connection.confirmTransaction(signature);
  });

  it("close accounts for Cancelled batch", async () => {
    const batchIndex = testMultisig.cancelledBatchIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    // First close the vault transactions.
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

    signature = await smartAccount.rpc.closeBatch({
      connection,
      feePayer: members.almighty,
      settingsPda,
      batchRentCollector: members.proposer.publicKey,
      proposalRentCollector: members.proposer.publicKey,
      batchIndex,
      programId,
    });
    await connection.confirmTransaction(signature);
  });
});
