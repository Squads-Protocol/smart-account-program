import * as smartAccount from "@sqds/smart-account";
import assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  generateSmartAccountSigners,
  getTestProgramId,
  TestMembers,
  getSignerKey,
  formatsToRun,
  getRpc,
} from "../../utils";

const { Settings, Proposal } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

for (const format of formatsToRun) {
  const rpc = getRpc(format);

  describe(`Instructions / settings_transaction_execute [${format}]`, () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("error: invalid proposal status (Rejected)", async () => {
    // Create new autonomous smartAccount.
    const settingsPda = (
      await createAutonomousMultisig({
        connection,
        members,
        threshold: 2,
        timeLock: 0,
        programId,
      })
    )[0];

    // Create a settings transaction.
    const transactionIndex = 1n;
    let signature = await rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 3 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction.
    signature = await rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Reject the proposal by a member.
    // Our threshold is 2 out of 2 voting members, so the cutoff is 1.
    signature = await rpc.rejectProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Attempt to execute a transaction with a rejected proposal.
    await assert.rejects(
      () =>
        rpc.executeSettingsTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionIndex,
          rentPayer: members.almighty,
          signer: members.almighty,
          programId,
        }),
      /Invalid proposal status/
    );
  });

  it("error: removing asignercauses threshold to be unreachable", async () => {
    // Create new autonomous smartAccount.
    const settingsPda = (
      await createAutonomousMultisig({
        connection,
        members,
        // Threshold is 2/2, we have just 2 voting members: almighty and voter.
        threshold: 2,
        timeLock: 0,
        programId,
      })
    )[0];

    // Create a settings transaction.
    const transactionIndex = 1n;
    let signature = await rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      // Try to remove 1 out of 2 voting members.
      actions: [{ __kind: "RemoveSigner", oldSigner: members.voter.publicKey }],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction.
    signature = await rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal 1.
    signature = await rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal 2.
    signature = await rpc.approveProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    await assert.rejects(
      () =>
        rpc.executeSettingsTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionIndex,
          signer: members.almighty,
          rentPayer: members.almighty,
          programId,
        }),
      /InvalidThreshold: Invalid threshold, must be between 1 and number of signers with vote permission/
    );
  });

  it("execute settings transaction with RemoveMember and ChangeThreshold actions", async () => {
    // Create new autonomous smartAccount.
    const settingsPda = (
      await createAutonomousMultisig({
        connection,
        members,
        // Threshold is 2/2, we have just 2 voting members: almighty and voter.
        threshold: 2,
        timeLock: 0,
        programId,
      })
    )[0];

    // Create a settings transaction.
    const transactionIndex = 1n;
    let signature = await rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      actions: [
        // Remove 1 out of 2 voting members.
        { __kind: "RemoveSigner", oldSigner: members.voter.publicKey },
        // and simultaneously change the threshold to 1/1.
        { __kind: "ChangeThreshold", newThreshold: 1 },
      ],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction.
    signature = await rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal 1.
    signature = await rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal 2.
    signature = await rpc.approveProposal({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    signature = await rpc.executeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      rentPayer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Verify the smart account account.
    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    // The threshold should have been updated.
    assert.strictEqual(multisigAccount.threshold, 1);
    // Voter should have been removed.
    assert(
      !multisigAccount.signers.some((m) =>
        getSignerKey(m).equals(members.voter.publicKey)
      )
    );
    // The stale transaction index should be updated and set to 1.
    assert.strictEqual(multisigAccount.staleTransactionIndex.toString(), "1");
  });

  it("execute settings transaction with ChangeThreshold action", async () => {
    // Create new autonomous smartAccount.
    const settingsPda = (
      await createAutonomousMultisig({
        connection,
        members,
        threshold: 1,
        timeLock: 0,
        programId,
      })
    )[0];

    // Create a settings transaction.
    const transactionIndex = 1n;
    let signature = await rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction.
    signature = await rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal.
    signature = await rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Execute the approved settings transaction.
    signature = await rpc.executeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      rentPayer: members.almighty,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Verify the proposal account.
    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });
    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(
      smartAccount.types.isProposalStatusExecuted(proposalAccount.status)
    );

    // Verify the smart account account.
    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    // The threshold should have been updated.
    assert.strictEqual(multisigAccount.threshold, 2);
    // The stale transaction index should be updated and set to 1.
    assert.strictEqual(multisigAccount.staleTransactionIndex.toString(), "1");
  });

  it("execute settings transaction with SetRentCollector action", async () => {
    // Create new autonomous smart account without rent_collector.
    const settingsPda = (
      await createAutonomousMultisig({
        connection,
        members,
        threshold: 1,
        timeLock: 0,
        programId,
      })
    )[0];

    const multisigAccountInfoPreExecution = await connection.getAccountInfo(
      settingsPda
    )!;

    const vaultPda = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    })[0];

    // Create a settings transaction.
    const transactionIndex = 1n;
    let signature = await rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      actions: [
        { __kind: "SetArchivalAuthority", newArchivalAuthority: vaultPda },
      ],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create a proposal for the transaction (Approved).
    signature = await rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal.
    signature = await rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Execute the approved settings transaction.
    await assert.rejects(
      () =>
        rpc.executeSettingsTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionIndex,
          signer: members.almighty,
          rentPayer: members.almighty,
          programId,
        }),
      /NotImplemented/
    );
    await connection.confirmTransaction(signature);
    // Reject the proposal.
    signature = await rpc.cancelProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Verify the proposal account.
    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });
    const proposalAccount = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(
      smartAccount.types.isProposalStatusCancelled(proposalAccount.status)
    );
  });
});
}
