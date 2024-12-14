import * as multisig from "@sqds/multisig";
import assert from "assert";
import {
    createAutonomousMultisig,
    createLocalhostConnection,
    generateMultisigMembers,
    getTestProgramId,
    TestMembers,
} from "../../utils";

const { Multisig, Proposal } = multisig.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / config_transaction_execute", () => {
    let members: TestMembers;

    before(async () => {
        members = await generateMultisigMembers(connection);
    });

    it("error: insufficient vote permissions", async () => {
        // Create new autonomous multisig.
        const multisigPda = (
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
                let signature = await multisig.rpc.configTransactionSync({
                    connection,
                    feePayer: members.proposer,
                    multisigPda,
                    signers: [members.proposer, members.voter, members.executor],
                    configActions: [{ __kind: "ChangeThreshold", newThreshold: 3 }],
                    programId,
                });


            },
            /InsufficientVotePermissions/
        )
    });

    it("error: not enough signers", async () => {
        // Create new autonomous multisig.
        const multisigPda = (
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
                let signature = await multisig.rpc.configTransactionSync({
                    connection,
                    feePayer: members.almighty,
                    multisigPda,
                    signers: [members.almighty],
                    configActions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
                    programId,
                });


            },
            /InvalidSignerCount/
        )
    });

    it("error: removing a member causes threshold to be unreachable", async () => {
        // Create new autonomous multisig.
        const multisigPda = (
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

                let signature = await multisig.rpc.configTransactionSync({
                    connection,
                    feePayer: members.voter,
                    multisigPda,
                    signers: [members.voter, members.almighty],
                    configActions: [
                        // Try to remove 1 out of 2 voting members.
                        { __kind: "RemoveMember", oldMember: members.voter.publicKey },
                    ],
                    programId,
                });

            },
            /InvalidThreshold/
        );
    });

    it("execute config transaction with RemoveMember and ChangeThreshold actions", async () => {
        // Create new autonomous multisig.
        const multisigPda = (
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
        let _signature = await multisig.rpc.configTransactionCreate({
            connection,
            creator: members.proposer.publicKey,
            transactionIndex: 1n,
            feePayer: members.proposer,
            multisigPda,
            actions: [
                { __kind: "RemoveMember", oldMember: members.voter.publicKey },
                { __kind: "ChangeThreshold", newThreshold: 1 },
            ],
            programId,
        });
        await connection.confirmTransaction(_signature);

        // Create a config transaction.
        let signature = await multisig.rpc.configTransactionSync({
            connection,
            feePayer: members.voter,
            multisigPda,
            signers: [members.voter, members.almighty],
            configActions: [
                // Remove 1 out of 2 voting members.
                { __kind: "RemoveMember", oldMember: members.voter.publicKey },
                // and simultaneously change the threshold to 1/1.
                { __kind: "ChangeThreshold", newThreshold: 1 },
            ],
            programId,
        });
        await connection.confirmTransaction(signature);

        // Verify the multisig account.
        const multisigAccount = await Multisig.fromAccountAddress(
            connection,
            multisigPda
        );
        // The threshold should have been updated.
        assert.strictEqual(multisigAccount.threshold, 1);
        // Voter should have been removed.
        assert(
            !multisigAccount.members.some((m) =>
                m.key.equals(members.voter.publicKey)
            )
        );
        // The stale transaction index should be updated and set to 1.
        assert.strictEqual(multisigAccount.staleTransactionIndex.toString(), "1");
    });

    it("execute config transaction with ChangeThreshold action", async () => {
        // Create new autonomous multisig.
        const multisigPda = (
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
        let _signature = await multisig.rpc.configTransactionCreate({
            connection,
            creator: members.proposer.publicKey,
            transactionIndex: 1n,
            feePayer: members.proposer,
            multisigPda,
            actions: [
                { __kind: "RemoveMember", oldMember: members.voter.publicKey },
                { __kind: "ChangeThreshold", newThreshold: 1 },
            ],
            programId,
        });
        await connection.confirmTransaction(_signature);

        // Execute a synchronous config transaction.
        let signature = await multisig.rpc.configTransactionSync({
            connection,
            feePayer: members.almighty,
            multisigPda,
            signers: [members.almighty],
            configActions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
            programId,
        });
        await connection.confirmTransaction(signature);

        // Verify the multisig account.
        const multisigAccount = await Multisig.fromAccountAddress(
            connection,
            multisigPda
        );
        // The threshold should have been updated.
        assert.strictEqual(multisigAccount.threshold, 2);
        // The stale transaction index should be updated and set to 1.
        assert.strictEqual(multisigAccount.staleTransactionIndex.toString(), "1");
    });

    it("execute config transaction with SetRentCollector action", async () => {
        // Create new autonomous multisig without rent_collector.
        const multisigPda = (
            await createAutonomousMultisig({
                connection,
                members,
                threshold: 1,
                timeLock: 0,
                programId,
            })
        )[0];

        const multisigAccountInfoPreExecution = await connection.getAccountInfo(
            multisigPda
        )!;

        const vaultPda = multisig.getVaultPda({
            multisigPda,
            index: 0,
            programId,
        })[0];

        // Create random config transaction
        // This is so we can check that the stale transaction index is not
        // after the synchronous change
        let _signature = await multisig.rpc.configTransactionCreate({
            connection,
            creator: members.proposer.publicKey,
            transactionIndex: 1n,
            feePayer: members.proposer,
            multisigPda,
            actions: [
                { __kind: "RemoveMember", oldMember: members.voter.publicKey },
                { __kind: "ChangeThreshold", newThreshold: 1 },
            ],
            programId,
        });
        await connection.confirmTransaction(_signature);

        // Create a config transaction.
        let signature = await multisig.rpc.configTransactionSync({
            connection,
            feePayer: members.almighty,
            multisigPda,
            signers: [members.almighty],
            configActions: [{ __kind: "SetRentCollector", newRentCollector: vaultPda }],
            programId,
        });
        await connection.confirmTransaction(signature);
        // Verify the multisig account.
        const multisigAccountInfoPostExecution = await connection.getAccountInfo(
            multisigPda
        );
        const [multisigAccountPostExecution] = Multisig.fromAccountInfo(
            multisigAccountInfoPostExecution!
        );
        // The rentCollector should be updated.
        assert.strictEqual(
            multisigAccountPostExecution.rentCollector?.toBase58(),
            vaultPda.toBase58()
        );
        // The stale transaction index should NOT be updated and remain 0.
        assert.strictEqual(
            multisigAccountPostExecution.staleTransactionIndex.toString(),
            "0"
        );
        // multisig space should not be reallocated because we allocate 32 bytes for potential rent_collector when we create multisig.
        assert.ok(
            multisigAccountInfoPostExecution!.data.length ===
            multisigAccountInfoPreExecution!.data.length
        );
    });
});
