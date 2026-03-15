import { Keypair, PublicKey } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import * as assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
  unwrapSigners,
  formatsToRun,
  getRpc,
} from "../../utils";

const { Settings, Proposal } = smartAccount.accounts;
const { Permission, Permissions } = smartAccount.types;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

for (const format of formatsToRun) {
  const rpc = getRpc(format);

  describe(`Instructions / proposal_reject [${format}]`, () => {
    let members: TestMembers;

    before(async () => {
      members = await generateSmartAccountSigners(connection);
    });

    describe("proposal_reject", () => {
      let settingsPda: PublicKey;

      before(async () => {
        const feePayer = await generateFundedKeypair(connection);
        const accountIndex = await getNextAccountIndex(connection, programId);

        // Create new autonomous smartAccount.
        settingsPda = (
          await createAutonomousMultisig({
            connection,
            accountIndex,
            members,
            threshold: 2,
            timeLock: 0,
            programId,
          })
        )[0];

        // Create first settings transaction.
        let signature = await rpc.createSettingsTransaction({
          connection,
          feePayer: members.proposer,
          settingsPda,
          transactionIndex: 1n,
          creator: members.proposer.publicKey,
          actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
          programId,
        });
        await connection.confirmTransaction(signature);

        // Create second settings transaction.
        signature = await rpc.createSettingsTransaction({
          connection,
          feePayer: members.proposer,
          settingsPda,
          transactionIndex: 2n,
          creator: members.proposer.publicKey,
          actions: [{ __kind: "SetTimeLock", newTimeLock: 60 }],
          programId,
        });
        await connection.confirmTransaction(signature);

        // Create a proposal for the first settings transaction.
        signature = await rpc.createProposal({
          connection,
          feePayer,
          settingsPda,
          transactionIndex: 1n,
          creator: members.proposer,
          programId,
        });
        await connection.confirmTransaction(signature);

        // Create a proposal for the second settings transaction.
        signature = await rpc.createProposal({
          connection,
          feePayer,
          settingsPda,
          transactionIndex: 2n,
          creator: members.proposer,
          programId,
        });
        await connection.confirmTransaction(signature);

        // Approve the proposal for the first settings transaction and reach the threshold.
        signature = await rpc.approveProposal({
          connection,
          feePayer: members.voter,
          settingsPda,
          transactionIndex: 1n,
          signer: members.voter,
          programId,
        });
        await connection.confirmTransaction(signature);
        signature = await rpc.approveProposal({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionIndex: 1n,
          signer: members.almighty,
          programId,
        });
        await connection.confirmTransaction(signature);
      });

      it("error: try to reject an approved proposal", async () => {
        // Reject the proposal for the first settings transaction.
        const transactionIndex = 1n;

        await assert.rejects(
          () =>
            rpc.rejectProposal({
              connection,
              feePayer: members.voter,
              settingsPda,
              transactionIndex,
              signer: members.voter,
              programId,
            }),
          /Invalid proposal status/
        );
        const proposalAccount = await Proposal.fromAccountAddress(
          connection,
          smartAccount.getProposalPda({
            settingsPda,
            transactionIndex,
            programId,
          })[0]
        );
        assert.ok(
          smartAccount.types.isProposalStatusApproved(proposalAccount.status)
        );
      });

      it("error: not a signer", async () => {
        const nonMember = await generateFundedKeypair(connection);

        // Reject the proposal for the second settings transaction.
        const transactionIndex = 2n;

        await assert.rejects(
          () =>
            rpc.rejectProposal({
              connection,
              feePayer: nonMember,
              settingsPda,
              transactionIndex,
              signer: nonMember,
              programId,
            }),
          /Provided pubkey is not a signer of the smart account/
        );
      });

      it("error: unauthorized", async () => {
        // Reject the proposal for the second settings transaction.
        const transactionIndex = 2n;

        await assert.rejects(
          () =>
            rpc.rejectProposal({
              connection,
              feePayer: members.executor,
              settingsPda,
              transactionIndex,
              signer: members.executor,
              programId,
            }),
          /Attempted to perform an unauthorized action/
        );
      });

      it("reject proposal and reach cutoff", async () => {
        let multisigAccount = await Settings.fromAccountAddress(
          connection,
          settingsPda
        );

        // Reject the proposal for the second settings transaction.
        const transactionIndex = 2n;

        const signature = await rpc.rejectProposal({
          connection,
          feePayer: members.voter,
          settingsPda,
          transactionIndex,
          signer: members.voter,
          memo: "LGTM",
          programId,
        });
        await connection.confirmTransaction(signature);

        // Fetch the Proposal account.
        const [proposalPda] = smartAccount.getProposalPda({
          settingsPda,
          transactionIndex,
          programId,
        });
        const proposalAccount = await Proposal.fromAccountAddress(
          connection,
          proposalPda
        );
        assert.deepEqual(proposalAccount.approved, []);
        assert.deepEqual(proposalAccount.rejected, [members.voter.publicKey]);
        assert.deepEqual(proposalAccount.cancelled, []);
        // Our threshold is 2, and 2 voters, so the cutoff is 1...
        assert.strictEqual(multisigAccount.threshold, 2);
        assert.strictEqual(
          unwrapSigners(multisigAccount.signers).filter((m) =>
            Permissions.has(m.permissions, Permission.Vote)
          ).length,
          2
        );
        // ...thus we've reached the cutoff, and the proposal is now Rejected.
        assert.ok(
          smartAccount.types.isProposalStatusRejected(proposalAccount.status)
        );
      });

      it("error: already rejected", async () => {
        // Reject the proposal for the second settings transaction.
        const transactionIndex = 2n;

        await assert.rejects(
          () =>
            rpc.rejectProposal({
              connection,
              feePayer: members.almighty,
              settingsPda,
              transactionIndex,
              signer: members.almighty,
              programId,
            }),
          /Invalid proposal status/
        );

        const proposalAccount = await Proposal.fromAccountAddress(
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
      });

      it("error: stale transaction");

      it("error: transaction is not for smart account");
    });
  });
}
