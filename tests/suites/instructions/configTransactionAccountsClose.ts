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
  createLocalhostConnection,
  generateFundedKeypair,
  generateMultisigMembers,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const { Settings, Proposal } = multisig.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / config_transaction_accounts_close", () => {
  let members: TestMembers;
  let settingsPda: PublicKey;
  const staleTransactionIndex = 1n;
  const staleNoProposalTransactionIndex = 2n;
  const executedTransactionIndex = 3n;
  const activeTransactionIndex = 4n;
  const approvedTransactionIndex = 5n;
  const rejectedTransactionIndex = 6n;
  const cancelledTransactionIndex = 7n;

  // Set up a multisig with config transactions.
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
    await createAutonomousMultisigV2({
      connection,
      createKey,
      members,
      threshold: 2,
      timeLock: 0,
      rentCollector: vaultPda,
      programId,
    });

    //region Stale
    // Create a config transaction (Stale).
    let signature = await multisig.rpc.createSettingsTransaction({
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
    signature = await multisig.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: staleTransactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);
    // This transaction will become stale when the second config transaction is executed.
    //endregion

    //region Stale and No Proposal
    // Create a config transaction (Stale and No Proposal).
    signature = await multisig.rpc.createSettingsTransaction({
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

    // This transaction will become stale when the config transaction is executed.
    //endregion

    //region Executed
    // Create a config transaction (Executed).
    signature = await multisig.rpc.createSettingsTransaction({
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
    signature = await multisig.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: executedTransactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal by the first member.
    signature = await multisig.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex: executedTransactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal by the second member.
    signature = await multisig.rpc.approveProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex: executedTransactionIndex,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Execute the transaction.
    signature = await multisig.rpc.executeSettingsTransaction({
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
    // Create a config transaction (Active).
    signature = await multisig.rpc.createSettingsTransaction({
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
    signature = await multisig.rpc.createProposal({
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
      multisig.getProposalPda({
        settingsPda,
        transactionIndex: activeTransactionIndex,
        programId,
      })[0]
    );
    assert.ok(multisig.types.isProposalStatusActive(proposalAccount.status));
    //endregion

    //region Approved
    // Create a config transaction (Approved).
    signature = await multisig.rpc.createSettingsTransaction({
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
    signature = await multisig.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: approvedTransactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal.
    signature = await multisig.rpc.approveProposal({
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
      multisig.getProposalPda({
        settingsPda,
        transactionIndex: approvedTransactionIndex,
        programId,
      })[0]
    );
    assert.ok(multisig.types.isProposalStatusApproved(proposalAccount.status));
    //endregion

    //region Rejected
    // Create a config transaction (Rejected).
    signature = await multisig.rpc.createSettingsTransaction({
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
    signature = await multisig.rpc.createProposal({
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
    signature = await multisig.rpc.rejectProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex: rejectedTransactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Reject the proposal by the second member.
    signature = await multisig.rpc.rejectProposal({
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
      multisig.getProposalPda({
        settingsPda,
        transactionIndex: rejectedTransactionIndex,
        programId,
      })[0]
    );
    assert.ok(multisig.types.isProposalStatusRejected(proposalAccount.status));
    //endregion

    //region Cancelled
    // Create a config transaction (Cancelled).
    signature = await multisig.rpc.createSettingsTransaction({
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
    signature = await multisig.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex: cancelledTransactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal.
    signature = await multisig.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex: cancelledTransactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Cancel the proposal (The proposal should be approved at this point).
    signature = await multisig.rpc.cancelProposal({
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
      multisig.getProposalPda({
        settingsPda,
        transactionIndex: cancelledTransactionIndex,
        programId,
      })[0]
    );
    assert.ok(multisig.types.isProposalStatusCancelled(proposalAccount.status));

    //endregion
  });

  it("error: rent reclamation is not enabled", async () => {
    // Create a multisig with rent reclamation disabled.
    const settingsPda = (
      await createAutonomousMultisigV2({
        connection,
        members,
        threshold: 2,
        timeLock: 0,
        rentCollector: null,
        programId,
      })
    )[0];

    // Create a config transaction.
    const transactionIndex = 1n;
    let signature = await multisig.rpc.createSettingsTransaction({
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
    signature = await multisig.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal by the first member.
    signature = await multisig.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal by the second member.
    signature = await multisig.rpc.approveProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Execute the transaction.
    signature = await multisig.rpc.executeSettingsTransaction({
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
        multisig.rpc.closeSettingsTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          rentCollector: Keypair.generate().publicKey,
          transactionIndex,
          programId,
        }),
      /RentReclamationDisabled: Rent reclamation is disabled for this smart account/
    );
  });

  it("error: invalid rent_collector", async () => {
    const transactionIndex = approvedTransactionIndex;

    const fakeRentCollector = Keypair.generate().publicKey;

    await assert.rejects(
      () =>
        multisig.rpc.closeSettingsTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          rentCollector: fakeRentCollector,
          transactionIndex,
          programId,
        }),
      /Invalid rent collector address/
    );
  });

  it("error: proposal is for another multisig", async () => {
    const vaultPda = multisig.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    })[0];

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
    // Create a config transaction for it.
    let signature = await multisig.rpc.createSettingsTransaction({
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
    signature = await multisig.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda: otherMultisig,
      transactionIndex: 1n,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Manually construct an instruction that uses the proposal account from the other multisig.
    const ix =
      multisig.generated.createCloseSettingsTransactionInstruction(
        {
          settings: settingsPda,
          rentCollector: vaultPda,
          proposal: multisig.getProposalPda({
            settingsPda: otherMultisig,
            transactionIndex: 1n,
            programId,
          })[0],
          transaction: multisig.getTransactionPda({
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
          .catch(multisig.errors.translateAndThrowAnchorError),
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
        multisig.rpc.closeSettingsTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          rentCollector: multisigAccount.rentCollector!,
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
        multisig.rpc.closeSettingsTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          rentCollector: multisigAccount.rentCollector!,
          transactionIndex,
          programId,
        }),
      /Invalid proposal status/
    );
  });

  it("error: transaction is for another multisig", async () => {
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
    // Create a config transaction for it.
    let signature = await multisig.rpc.createSettingsTransaction({
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
    signature = await multisig.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda: otherMultisig,
      transactionIndex: 1n,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    const vaultPda = multisig.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    })[0];

    const feePayer = await generateFundedKeypair(connection);

    // Manually construct an instruction that uses transaction that doesn't match proposal.
    const ix =
      multisig.generated.createCloseSettingsTransactionInstruction(
        {
          settings: settingsPda,
          rentCollector: vaultPda,
          proposal: multisig.getProposalPda({
            settingsPda,
            transactionIndex: 1n,
            programId,
          })[0],
          transaction: multisig.getTransactionPda({
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
          .catch(multisig.errors.translateAndThrowAnchorError),
      /Transaction is for another smart account/
    );
  });

  it("error: transaction doesn't match proposal", async () => {
    const feePayer = await generateFundedKeypair(connection);

    const vaultPda = multisig.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    })[0];

    // Manually construct an instruction that uses transaction that doesn't match proposal.
    const ix =
      multisig.generated.createCloseSettingsTransactionInstruction(
        {
          settings: settingsPda,
          rentCollector: vaultPda,
          proposal: multisig.getProposalPda({
            settingsPda,
            transactionIndex: rejectedTransactionIndex,
            programId,
          })[0],
          transaction: multisig.getTransactionPda({
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
          .catch(multisig.errors.translateAndThrowAnchorError),
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
      multisig.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.ok(multisig.types.isProposalStatusActive(proposalAccount.status));

    // Make sure the proposal is stale.
    assert.ok(
      proposalAccount.transactionIndex <= multisigAccount.staleTransactionIndex
    );

    const [vaultPda] = multisig.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    const preBalance = await connection.getBalance(vaultPda);

    const sig = await multisig.rpc.closeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      rentCollector: vaultPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(sig);

    const postBalance = await connection.getBalance(vaultPda);
    const accountsRent = 5554080;
    assert.ok(postBalance === preBalance + accountsRent);
  });

  it("close accounts for Stale transaction with No Proposal", async () => {
    const transactionIndex = staleNoProposalTransactionIndex;

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    // Make sure there's no proposal.
    let proposalAccount = await connection.getAccountInfo(
      multisig.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.equal(proposalAccount, null);

    // Make sure the transaction is stale.
    assert.ok(
      transactionIndex <=
      multisig.utils.toBigInt(multisigAccount.staleTransactionIndex)
    );

    const [vaultPda] = multisig.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    const preBalance = await connection.getBalance(vaultPda);

    const sig = await multisig.rpc.closeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      rentCollector: vaultPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(sig);

    const postBalance = await connection.getBalance(vaultPda);
    const accountsRent = 1_503_360; // Rent for the transaction account.
    assert.equal(postBalance, preBalance + accountsRent);
  });

  it("close accounts for Executed transaction", async () => {
    const transactionIndex = executedTransactionIndex;

    // Make sure the proposal is Executed.
    let proposalAccount = await Proposal.fromAccountAddress(
      connection,
      multisig.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.ok(multisig.types.isProposalStatusExecuted(proposalAccount.status));

    const [vaultPda] = multisig.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    const preBalance = await connection.getBalance(vaultPda);

    const sig = await multisig.rpc.closeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      rentCollector: vaultPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(sig);

    const postBalance = await connection.getBalance(vaultPda);
    const accountsRent = 5554080;
    assert.ok(postBalance === preBalance + accountsRent);
  });

  it("close accounts for Rejected transaction", async () => {
    const transactionIndex = rejectedTransactionIndex;

    // Make sure the proposal is Rejected.
    let proposalAccount = await Proposal.fromAccountAddress(
      connection,
      multisig.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.ok(multisig.types.isProposalStatusRejected(proposalAccount.status));

    const [vaultPda] = multisig.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    const preBalance = await connection.getBalance(vaultPda);

    const sig = await multisig.rpc.closeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      rentCollector: vaultPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(sig);

    const postBalance = await connection.getBalance(vaultPda);
    const accountsRent = 5554080;
    assert.ok(postBalance === preBalance + accountsRent);
  });

  it("close accounts for Cancelled transaction", async () => {
    const transactionIndex = cancelledTransactionIndex;

    // Make sure the proposal is Cancelled.
    let proposalAccount = await Proposal.fromAccountAddress(
      connection,
      multisig.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0]
    );
    assert.ok(multisig.types.isProposalStatusCancelled(proposalAccount.status));

    const [vaultPda] = multisig.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    const preBalance = await connection.getBalance(vaultPda);

    const sig = await multisig.rpc.closeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      rentCollector: vaultPda,
      transactionIndex,
      programId,
    });
    await connection.confirmTransaction(sig);

    const postBalance = await connection.getBalance(vaultPda);
    const accountsRent = 5554080;
    assert.ok(postBalance === preBalance + accountsRent);
  });
});
