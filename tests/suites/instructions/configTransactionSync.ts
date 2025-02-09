import * as multisig from "@sqds/multisig";
import assert from "assert";
import {
    createAutonomousMultisig,
    createLocalhostConnection,
    generateMultisigMembers,
    getTestProgramId,
    TestMembers,
} from "../../utils";
import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";

const { Settings, Proposal } = multisig.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / config_transaction_execute", () => {
    let members: TestMembers;

    before(async () => {
        members = await generateMultisigMembers(connection);
    });

    it("error: insufficient vote permissions", async () => {
        // Create new autonomous multisig.
        const settingsPda = (
            await createAutonomousMultisig({
                connection,
                members,
                threshold: 2,
                timeLock: 0,
                programId,
            })
        )[0];

        // Create a config transaction.
        await assert.rejects(
            async () => {
                let signature = await multisig.rpc.executeSettingsTransactionSync({
                    connection,
                    feePayer: members.proposer,
                    settingsPda,
                    signers: [members.proposer, members.voter, members.executor],
                    actions: [{ __kind: "ChangeThreshold", newThreshold: 3 }],
                    programId,
                });


            },
            /InsufficientVotePermissions/
        )
    });

    it("error: not enough signers", async () => {
        // Create new autonomous multisig.
        const settingsPda = (
            await createAutonomousMultisig({
                connection,
                members,
                threshold: 2,
                timeLock: 0,
                programId,
            })
        )[0];

        // Create a config transaction.
        await assert.rejects(
            async () => {
                let signature = await multisig.rpc.executeSettingsTransactionSync({
                    connection,
                    feePayer: members.almighty,
                    settingsPda,
                    signers: [members.almighty],
                    actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
                    programId,
                });


            },
            /InvalidSignerCount/
        )
    });

    it("error: removing a member causes threshold to be unreachable", async () => {
        // Create new autonomous multisig.
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

        await assert.rejects(
            async () => {

                let signature = await multisig.rpc.executeSettingsTransactionSync({
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

            },
            /InvalidThreshold/
        );
    });

    it("execute config transaction with RemoveMember and ChangeThreshold actions", async () => {
        // Create new autonomous multisig.
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
        // Create random config transaction
        // This is so we can check that the stale transaction index is updated
        // after the synchronous change
        let _signature = await multisig.rpc.createSettingsTransaction({
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

        // Create a config transaction.
        let signature = await multisig.rpc.executeSettingsTransactionSync({
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

        // Verify the multisig account.
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

    it("execute config transaction with ChangeThreshold action", async () => {
        // Create new autonomous multisig.
        const settingsPda = (
            await createAutonomousMultisig({
                connection,
                members,
                threshold: 1,
                timeLock: 0,
                programId,
            })
        )[0];
        // Create random config transaction
        // This is so we can check that the stale transaction index is updated
        // after the synchronous change
        let _signature = await multisig.rpc.createSettingsTransaction({
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

        // Execute a synchronous config transaction.
        let signature = await multisig.rpc.executeSettingsTransactionSync({
            connection,
            feePayer: members.almighty,
            settingsPda,
            signers: [members.almighty],
            actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
            programId,
        });
        await connection.confirmTransaction(signature);

        // Verify the multisig account.
        const multisigAccount = await Settings.fromAccountAddress(
            connection,
            settingsPda
        );
        // The threshold should have been updated.
        assert.strictEqual(multisigAccount.threshold, 2);
        // The stale transaction index should be updated and set to 1.
        assert.strictEqual(multisigAccount.staleTransactionIndex.toString(), "1");
    });

    it("execute config transaction with SetRentCollector action", async () => {
        // Create new autonomous multisig without rent_collector.
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

        const vaultPda = multisig.getSmartAccountPda({
            settingsPda,
            accountIndex: 0,
            programId,
        })[0];

        // Create random config transaction
        // This is so we can check that the stale transaction index is not
        // after the synchronous change
        let _signature = await multisig.rpc.createSettingsTransaction({
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

        // Create a config transaction.
        await assert.rejects(
            async () => {
                let signature = await multisig.rpc.executeSettingsTransactionSync({
                    connection,
                    feePayer: members.almighty,
                    settingsPda,
                    signers: [members.almighty],
                    actions: [{ __kind: "SetArchivalAuthority", newArchivalAuthority: vaultPda }],
                    programId,
                });
            },
            /NotImplemented/
        );
    });
    it("should add a spending limit", async () => {
        // Create new autonomous multisig
        const settingsPda = (
            await createAutonomousMultisig({
                connection,
                members,
                threshold: 1,
                timeLock: 0,
                programId,
            })
        )[0];

        const spendingLimitSeed = members.almighty.publicKey;
        const [spendingLimitPda] = multisig.getSpendingLimitPda({
            settingsPda,
            seed: spendingLimitSeed,
            programId,
        });

        // Create and execute spending limit in one transaction
        const signature = await multisig.rpc.executeSettingsTransactionSync({
          connection,
          feePayer: members.almighty,
          settingsPda,
          signers: [members.almighty],
          actions: [
            {
            __kind: "AddSpendingLimit",
              seed: spendingLimitSeed,
              accountIndex: 0,
              mint: PublicKey.default, // SOL
              amount: 1_000_000_000, // 1 SOL
              period: 0,
              destinations: [members.almighty.publicKey],
              signers: [members.almighty.publicKey],
              expiration: new BN("9223372036854775807"), // i64 max (no expiration)
            },
          ],
          programId,
          remainingAccounts: [
            {pubkey: spendingLimitPda, isSigner: false, isWritable: true}
          ],
          sendOptions: {
            skipPreflight: true,
          },
        });

        await connection.confirmTransaction(signature);

        // Verify spending limit was created
        const spendingLimitAccount = await connection.getAccountInfo(spendingLimitPda);
        assert(spendingLimitAccount !== null, "Spending limit account should exist");
    });
});
