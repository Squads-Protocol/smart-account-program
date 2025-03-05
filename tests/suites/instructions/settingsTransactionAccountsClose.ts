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
  createLocalhostConnection,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const { Settings, Proposal } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / settings_transaction_accounts_close", () => {
  let members: TestMembers;
  let settingsPda: PublicKey;
  const staleTransactionIndex = 1n;
  const staleNoProposalTransactionIndex = 2n;
  const executedTransactionIndex = 3n;
  const activeTransactionIndex = 4n;
  const approvedTransactionIndex = 5n;
  const rejectedTransactionIndex = 6n;
  const cancelledTransactionIndex = 7n;

  // Set up a smart account with config transactions.
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
    await createAutonomousSmartAccountV2({
      connection,
      accountIndex,
      members,
      threshold: 2,
      timeLock: 0,
      rentCollector: vaultPda,
      programId,
    });

    //region Stale
    // Create a settings transaction (Stale).
    let signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: staleTransactionIndex,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction (Stale).
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: staleTransactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);
    // This transaction will become stale when the second settings transaction is executed.
    //endregion

    //region Stale and No Proposal
    // Create a settings transaction (Stale and No Proposal).
    signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: staleNoProposalTransactionIndex,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    // No proposal created for this transaction.

    // This transaction will become stale when the settings transaction is executed.
    //endregion

    //region Executed
    // Create a settings transaction (Executed).
    signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: executedTransactionIndex,
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
      transactionIndex: executedTransactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal by the first member.
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex: executedTransactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal by the second member.
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex: executedTransactionIndex,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Execute the transaction.
    signature = await smartAccount.rpc.executeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex: executedTransactionIndex,
      signer: members.almighty,
      rentPayer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    //endregion

    //region Active
    // Create a settings transaction (Active).
    signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: activeTransactionIndex,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction (Active).
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: activeTransactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Make sure the proposal is active.
    let proposalAccount = await Proposal.fromAccountAddress(
      connection,
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex: activeTransactionIndex,
        programId,
      })[0]
    );
    assert.ok(
      smartAccount.types.isProposalStatusActive(proposalAccount.status)
    );
    //endregion

    //region Approved
    // Create a settings transaction (Approved).
    signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: approvedTransactionIndex,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction (Approved).
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: approvedTransactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal.
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex: approvedTransactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Make sure the proposal is approved.
    proposalAccount = await Proposal.fromAccountAddress(
      connection,
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex: approvedTransactionIndex,
        programId,
      })[0]
    );
    assert.ok(
      smartAccount.types.isProposalStatusApproved(proposalAccount.status)
    );
    //endregion

    //region Rejected
    // Create a settings transaction (Rejected).
    signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: rejectedTransactionIndex,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 3 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction (Rejected).
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: rejectedTransactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Our threshold is 1, and 2 voters, so the cutoff is 2...

    // Reject the proposal by the first member.
    signature = await smartAccount.rpc.rejectProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex: rejectedTransactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Reject the proposal by the second member.
    signature = await smartAccount.rpc.rejectProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex: rejectedTransactionIndex,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Make sure the proposal is rejected.
    proposalAccount = await Proposal.fromAccountAddress(
      connection,
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex: rejectedTransactionIndex,
        programId,
      })[0]
    );
    assert.ok(
      smartAccount.types.isProposalStatusRejected(proposalAccount.status)
    );
    //endregion

    //region Cancelled
    // Create a settings transaction (Cancelled).
    signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: cancelledTransactionIndex,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 3 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction (Cancelled).
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: cancelledTransactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal.
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex: cancelledTransactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Cancel the proposal (The proposal should be approved at this point).
    signature = await smartAccount.rpc.cancelProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex: cancelledTransactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Make sure the proposal is cancelled.
    proposalAccount = await Proposal.fromAccountAddress(
      connection,
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex: cancelledTransactionIndex,
        programId,
      })[0]
    );
    assert.ok(
      smartAccount.types.isProposalStatusCancelled(proposalAccount.status)
    );

    //endregion
  });

  it("error: invalid transaction rent_collector", async () => {
    // Create a smart account with rent reclamation disabled.
    const accountIndex = await getNextAccountIndex(connection, programId);
    const settingsPda = (
      await createAutonomousSmartAccountV2({
        connection,
        members,
        threshold: 2,
        timeLock: 0,
        accountIndex,
        rentCollector: null,
        programId,
      })
    )[0];

    // Create a settings transaction.
    const transactionIndex = 1n;
    let signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction.
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal by the first member.
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal by the second member.
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Execute the transaction.
    signature = await smartAccount.rpc.executeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      rentPayer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Attempt to close the accounts.
    await assert.rejects(
      () =>
        smartAccount.rpc.closeSettingsTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionRentCollector: Keypair.generate().publicKey,
          proposalRentCollector: members.proposer.publicKey,
          transactionIndex,
          programId,
        }),
      /InvalidRentCollector/
    );
  });

  it("error: invalid proposal rent_collector", async () => {
    const transactionIndex = 1n;

    const fakeRentCollector = Keypair.generate().publicKey;

    await assert.rejects(
      () =>
        smartAccount.rpc.closeSettingsTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionRentCollector: members.proposer.publicKey,
          proposalRentCollector: fakeRentCollector,
          transactionIndex,
          programId,
        }),
      /InvalidRentCollector/
    );
  });

  it("error: proposal is for another smart account", async () => {
    const vaultPda = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    })[0];
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
    // Create a settings transaction for it.
    let signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda: otherMultisig,
      transactionIndex: 1n,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
      programId,
    });
    await connection.confirmTransaction(signature);
    // Create a proposal for it.
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda: otherMultisig,
      transactionIndex: 1n,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Manually construct an instruction that uses the proposal account from the other smartAccount.
    const ix = smartAccount.generated.createCloseSettingsTransactionInstruction(
      {
        settings: settingsPda,
        transactionRentCollector: members.proposer.publicKey,
        proposalRentCollector: members.proposer.publicKey,
        proposal: smartAccount.getProposalPda({
          settingsPda: otherMultisig,
          transactionIndex: 1n,
          programId,
        })[0],
        transaction: smartAccount.getTransactionPda({
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
      /A seeds constraint was violated/
    );
  });

  it("error: invalid proposal status (Active)", async () => {
    const transactionIndex = activeTransactionIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    await assert.rejects(
      () =>
        smartAccount.rpc.closeSettingsTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionRentCollector: members.proposer.publicKey,
          proposalRentCollector: members.proposer.publicKey,
          transactionIndex,
          programId,
        }),
      /Invalid proposal status/
    );
  });

  it("error: invalid proposal status (Approved)", async () => {
    const transactionIndex = approvedTransactionIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    await assert.rejects(
      () =>
        smartAccount.rpc.closeSettingsTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionRentCollector: members.proposer.publicKey,
          proposalRentCollector: members.proposer.publicKey,
          transactionIndex,
          programId,
        }),
      /Invalid proposal status/
    );
  });

  it("error: transaction is for another smart account", async () => {
    // Create another smartAccount.
    const accountIndex = await getNextAccountIndex(connection, programId);
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
    // Create a settings transaction for it.
    let signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda: otherMultisig,
      transactionIndex: 1n,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
      programId,
    });
    await connection.confirmTransaction(signature);
    // Create a proposal for it.
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda: otherMultisig,
      transactionIndex: 1n,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    const vaultPda = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    })[0];

    const feePayer = await generateFundedKeypair(connection);

    // Manually construct an instruction that uses transaction that doesn't match proposal.
    const ix = smartAccount.generated.createCloseSettingsTransactionInstruction(
      {
        settings: settingsPda,
        transactionRentCollector: members.proposer.publicKey,
        proposalRentCollector: members.proposer.publicKey,
        proposal: smartAccount.getProposalPda({
          settingsPda,
          transactionIndex: 1n,
          programId,
        })[0],
        transaction: smartAccount.getTransactionPda({
          settingsPda: otherMultisig,
          transactionIndex: 1n,
          programId,
        })[0],
      },
      programId
    );

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

  it("error: transaction doesn't match proposal", async () => {
    const feePayer = await generateFundedKeypair(connection);

    const vaultPda = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    })[0];

    // Manually construct an instruction that uses transaction that doesn't match proposal.
    const ix = smartAccount.generated.createCloseSettingsTransactionInstruction(
      {
        settings: settingsPda,
        transactionRentCollector: members.proposer.publicKey,
        proposalRentCollector: members.proposer.publicKey,
        proposal: smartAccount.getProposalPda({
          settingsPda,
          transactionIndex: rejectedTransactionIndex,
          programId,
        })[0],
        transaction: smartAccount.getTransactionPda({
          settingsPda,
          // Wrong transaction index.
          transactionIndex: approvedTransactionIndex,
          programId,
        })[0],
      },
      programId
    );

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
      /A seeds constraint was violated/
    );
  });

  it("close accounts for Stale transaction", async () => {
    const transactionIndex = staleTransactionIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    // Make sure the proposal is still active.
    let proposalAccount = await Proposal.fromAccountAddress(
      connection,
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.ok(
      smartAccount.types.isProposalStatusActive(proposalAccount.status)
    );

    // Make sure the proposal is stale.
    assert.ok(
      proposalAccount.transactionIndex <= multisigAccount.staleTransactionIndex
    );

    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    const preBalance = await connection.getBalance(members.proposer.publicKey);

    const sig = await smartAccount.rpc.closeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionRentCollector: members.proposer.publicKey,
      proposalRentCollector: members.proposer.publicKey,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(sig);

    const postBalance = await connection.getBalance(members.proposer.publicKey);
    assert.ok(postBalance > preBalance);
  });

  it("close accounts for Stale transaction with No Proposal", async () => {
    const transactionIndex = staleNoProposalTransactionIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    // Make sure there's no proposal.
    let proposalAccount = await connection.getAccountInfo(
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.equal(proposalAccount, null);

    // Make sure the transaction is stale.
    assert.ok(
      transactionIndex <=
        smartAccount.utils.toBigInt(multisigAccount.staleTransactionIndex)
    );

    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    const preBalance = await connection.getBalance(members.proposer.publicKey);

    const sig = await smartAccount.rpc.closeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionRentCollector: members.proposer.publicKey,
      proposalRentCollector: members.proposer.publicKey,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(sig);

    const postBalance = await connection.getBalance(members.proposer.publicKey);
    assert.ok(postBalance > preBalance);
  });

  it("close accounts for Executed transaction", async () => {
    const transactionIndex = executedTransactionIndex;

    // Make sure the proposal is Executed.
    let proposalAccount = await Proposal.fromAccountAddress(
      connection,
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.ok(
      smartAccount.types.isProposalStatusExecuted(proposalAccount.status)
    );

    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    const preBalance = await connection.getBalance(members.proposer.publicKey);

    const sig = await smartAccount.rpc.closeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionRentCollector: members.proposer.publicKey,
      proposalRentCollector: members.proposer.publicKey,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(sig);

    const postBalance = await connection.getBalance(members.proposer.publicKey);
    assert.ok(postBalance > preBalance);
  });

  it("close accounts for Rejected transaction", async () => {
    const transactionIndex = rejectedTransactionIndex;

    // Make sure the proposal is Rejected.
    let proposalAccount = await Proposal.fromAccountAddress(
      connection,
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.ok(
      smartAccount.types.isProposalStatusRejected(proposalAccount.status)
    );

    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    const preBalance = await connection.getBalance(members.proposer.publicKey);

    const sig = await smartAccount.rpc.closeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionRentCollector: members.proposer.publicKey,
      proposalRentCollector: members.proposer.publicKey,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(sig);

    const postBalance = await connection.getBalance(members.proposer.publicKey);
    assert.ok(postBalance > preBalance);
  });

  it("close accounts for Cancelled transaction", async () => {
    const transactionIndex = cancelledTransactionIndex;

    // Make sure the proposal is Cancelled.
    let proposalAccount = await Proposal.fromAccountAddress(
      connection,
      smartAccount.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.ok(
      smartAccount.types.isProposalStatusCancelled(proposalAccount.status)
    );

    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    const preBalance = await connection.getBalance(members.proposer.publicKey);

    const sig = await smartAccount.rpc.closeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionRentCollector: members.proposer.publicKey,
      proposalRentCollector: members.proposer.publicKey,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(sig);

    const postBalance = await connection.getBalance(members.proposer.publicKey);
    assert.ok(postBalance > preBalance);
  });
});
