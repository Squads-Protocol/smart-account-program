import { Keypair, PublicKey } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import * as assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  createSignerArray,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
  formatsToRun,
  getRpc,
} from "../../utils";

const { Proposal } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

for (const format of formatsToRun) {
  const rpc = getRpc(format);

  describe(`Instructions / proposal_cancel [${format}]`, () => {
    let members: TestMembers;

    before(async () => {
      members = await generateSmartAccountSigners(connection);
    });

    describe("proposal_cancel", () => {
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

        // Create a settings transaction.
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

        // Create a proposal for the settings transaction.
        signature = await rpc.createProposal({
          connection,
          feePayer,
          settingsPda,
          transactionIndex: 1n,
          creator: members.proposer,
          programId,
        });
        await connection.confirmTransaction(signature);

        // Approve the proposal for the settings transaction and reach the threshold.
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

        // The proposal must be `Approved` now.
        const [proposalPda] = smartAccount.getProposalPda({
          settingsPda,
          transactionIndex: 1n,
          programId,
        });
        let proposalAccount = await Proposal.fromAccountAddress(
          connection,
          proposalPda
        );
        assert.ok(
          smartAccount.types.isProposalStatusApproved(proposalAccount.status)
        );
      });

      it("cancel proposal", async () => {
        const transactionIndex = 1n;

        // Now cancel the proposal.
        let signature = await rpc.cancelProposal({
          connection,
          feePayer: members.voter,
          settingsPda,
          transactionIndex,
          signer: members.voter,
          programId,
        });
        await connection.confirmTransaction(signature);

        const proposalPda = smartAccount.getProposalPda({
          settingsPda,
          transactionIndex,
          programId,
        })[0];
        let proposalAccount = await Proposal.fromAccountAddress(
          connection,
          proposalPda
        );
        // Our threshold is 2, so after the first cancel, the proposal is still `Approved`.
        assert.ok(
          smartAccount.types.isProposalStatusApproved(proposalAccount.status)
        );

        // Second signer cancels the transaction.
        signature = await rpc.cancelProposal({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionIndex,
          signer: members.almighty,
          programId,
        });
        await connection.confirmTransaction(signature);

        proposalAccount = await Proposal.fromAccountAddress(
          connection,
          proposalPda
        );
        // Reached the threshold, so the transaction should be `Cancelled` now.
        assert.ok(
          smartAccount.types.isProposalStatusCancelled(proposalAccount.status)
        );
      });

      it("proposal_cancel_v2", async () => {
        // Create a settings transaction.
        const transactionIndex = 2n;
        let newVotingMember = new Keypair();

        const [proposalPda] = smartAccount.getProposalPda({
          settingsPda,
          transactionIndex,
          programId,
        });

        let signature = await rpc.createSettingsTransaction({
          connection,
          feePayer: members.proposer,
          settingsPda,
          transactionIndex,
          creator: members.proposer.publicKey,
          actions: [
            {
              __kind: "AddSigner",
              newSigner: createSignerArray(
                newVotingMember.publicKey,
                smartAccount.types.Permissions.all()
              ),
            },
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

        let proposalAccount = await Proposal.fromAccountAddress(
          connection,
          proposalPda
        );
        // Our threshold is 2, so after the first cancel, the proposal is still `Approved`.
        assert.ok(
          smartAccount.types.isProposalStatusApproved(proposalAccount.status)
        );

        // Proposal is now ready to execute, cast the 2 cancels using the new functionality.
        signature = await rpc.cancelProposal({
          connection,
          feePayer: members.voter,
          signer: members.voter,
          settingsPda,
          transactionIndex,
          programId,
        });
        await connection.confirmTransaction(signature);

        // Proposal is now ready to execute, cast the 2 cancels using the new functionality.
        signature = await rpc.cancelProposal({
          connection,
          feePayer: members.almighty,
          signer: members.almighty,
          settingsPda,
          transactionIndex,
          programId,
        });
        await connection.confirmTransaction(signature);

        // Proposal status must be "Cancelled".
        proposalAccount = await Proposal.fromAccountAddress(
          connection,
          proposalPda
        );
        assert.ok(
          smartAccount.types.isProposalStatusCancelled(proposalAccount.status)
        );
      });
    });
  });
}
