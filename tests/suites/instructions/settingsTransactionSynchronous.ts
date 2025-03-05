import * as smartAccount from "@sqds/smart-account";
import assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  generateSmartAccountSigners,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const { Settings, Proposal } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / settings_transaction_execute_sync", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("error: insufficient vote permissions", async () => {
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
    await assert.rejects(async () => {
      let signature = await smartAccount.rpc.executeSettingsTransactionSync({
        connection,
        feePayer: members.proposer,
        settingsPda,
        signers: [members.proposer, members.voter, members.executor],
        actions: [{ __kind: "ChangeThreshold", newThreshold: 3 }],
        programId,
      });
    }, /InsufficientVotePermissions/);
  });

  it("error: not enough signers", async () => {
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
    await assert.rejects(async () => {
      let signature = await smartAccount.rpc.executeSettingsTransactionSync({
        connection,
        feePayer: members.almighty,
        settingsPda,
        signers: [members.almighty],
        actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
        programId,
      });
    }, /InvalidSignerCount/);
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

    await assert.rejects(async () => {
      let signature = await smartAccount.rpc.executeSettingsTransactionSync({
        connection,
        feePayer: members.voter,
        settingsPda,
        signers: [members.voter, members.almighty],
        actions: [
          // Try to remove 1 out of 2 voting members.
          { __kind: "RemoveSigner", oldSigner: members.voter.publicKey },
        ],
        programId,
      });
    }, /InvalidThreshold/);
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
    // Create random settings transaction
    // This is so we can check that the stale transaction index is updated
    // after the synchronous change
    let _signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      creator: members.proposer.publicKey,
      transactionIndex: 1n,
      feePayer: members.proposer,
      settingsPda,
      actions: [
        { __kind: "RemoveSigner", oldSigner: members.voter.publicKey },
        { __kind: "ChangeThreshold", newThreshold: 1 },
      ],
      programId,
    });
    await connection.confirmTransaction(_signature);

    // Create a settings transaction.
    let signature = await smartAccount.rpc.executeSettingsTransactionSync({
      connection,
      feePayer: members.voter,
      settingsPda,
      signers: [members.voter, members.almighty],
      actions: [
        // Remove 1 out of 2 voting members.
        { __kind: "RemoveSigner", oldSigner: members.voter.publicKey },
        // and simultaneously change the threshold to 1/1.
        { __kind: "ChangeThreshold", newThreshold: 1 },
      ],
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
        m.key.equals(members.voter.publicKey)
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
    // Create random settings transaction
    // This is so we can check that the stale transaction index is updated
    // after the synchronous change
    let _signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      creator: members.proposer.publicKey,
      transactionIndex: 1n,
      feePayer: members.proposer,
      settingsPda,
      actions: [
        { __kind: "RemoveSigner", oldSigner: members.voter.publicKey },
        { __kind: "ChangeThreshold", newThreshold: 1 },
      ],
      programId,
    });
    await connection.confirmTransaction(_signature);

    // Execute a synchronous settings transaction.
    let signature = await smartAccount.rpc.executeSettingsTransactionSync({
      connection,
      feePayer: members.almighty,
      settingsPda,
      signers: [members.almighty],
      actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
      programId,
    });
    await connection.confirmTransaction(signature);

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

    // Create random settings transaction
    // This is so we can check that the stale transaction index is not
    // after the synchronous change
    let _signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      creator: members.proposer.publicKey,
      transactionIndex: 1n,
      feePayer: members.proposer,
      settingsPda,
      actions: [
        { __kind: "RemoveSigner", oldSigner: members.voter.publicKey },
        { __kind: "ChangeThreshold", newThreshold: 1 },
      ],
      programId,
    });
    await connection.confirmTransaction(_signature);

    // Create a settings transaction.
    await assert.rejects(async () => {
      let signature = await smartAccount.rpc.executeSettingsTransactionSync({
        connection,
        feePayer: members.almighty,
        settingsPda,
        signers: [members.almighty],
        actions: [
          { __kind: "SetArchivalAuthority", newArchivalAuthority: vaultPda },
        ],
        programId,
      });
    }, /NotImplemented/);
  });
});
