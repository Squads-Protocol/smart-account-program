import { Keypair, PublicKey } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import * as assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  comparePubkeys,
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

  describe(`Instructions / proposal_approve [${format}]`, () => {
    let members: TestMembers;

    before(async () => {
      members = await generateSmartAccountSigners(connection);
    });

    describe("proposal_approve", () => {
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

        const transactionIndex = 1n;

        // Create a settings transaction.
        let signature = await rpc.createSettingsTransaction({
          connection,
          feePayer: members.proposer,
          settingsPda,
          transactionIndex,
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
          transactionIndex,
          creator: members.proposer,
          programId,
        });
        await connection.confirmTransaction(signature);
      });

      it("error: not a signer", async () => {
        const nonMember = await generateFundedKeypair(connection);

        const transactionIndex = 1n;

        // Non-member cannot approve the proposal.
        await assert.rejects(
          () =>
            rpc.approveProposal({
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
        const transactionIndex = 1n;

        // Executor is not authorized to approve config transactions.
        await assert.rejects(
          () =>
            rpc.approveProposal({
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

      it("approve settings transaction", async () => {
        // Approve the proposal for the first settings transaction.
        const transactionIndex = 1n;

        const signature = await rpc.approveProposal({
          connection,
          feePayer: members.voter,
          settingsPda,
          transactionIndex,
          signer: members.voter,
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

        // Assertions.
        assert.deepEqual(proposalAccount.approved, [members.voter.publicKey]);
        assert.deepEqual(proposalAccount.rejected, []);
        assert.deepEqual(proposalAccount.cancelled, []);
        // Our threshold is 2, so the proposal is not yet Approved.
        assert.ok(
          smartAccount.types.isProposalStatusActive(proposalAccount.status)
        );
      });

      it("error: already approved", async () => {
        // Approve the proposal for the first settings transaction once again.
        const transactionIndex = 1n;

        await assert.rejects(
          () =>
            rpc.approveProposal({
              connection,
              feePayer: members.voter,
              settingsPda,
              transactionIndex,
              signer: members.voter,
              programId,
            }),
          /Signer already approved the transaction/
        );
      });

      it("approve settings transaction and reach threshold", async () => {
        // Approve the proposal for the first settings transaction.
        const transactionIndex = 1n;

        const signature = await rpc.approveProposal({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionIndex,
          signer: members.almighty,
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

        // Assertions.
        assert.deepEqual(
          proposalAccount.approved.map((key) => key.toBase58()),
          [members.voter.publicKey, members.almighty.publicKey]
            .sort(comparePubkeys)
            .map((key) => key.toBase58())
        );
        assert.deepEqual(proposalAccount.rejected, []);
        assert.deepEqual(proposalAccount.cancelled, []);
        // Our threshold is 2, so the transaction is now Approved.
        assert.ok(
          smartAccount.types.isProposalStatusApproved(proposalAccount.status)
        );
      });

      it("error: stale transaction");

      it("error: invalid transaction status");

      it("error: proposal is not for smart account");
    });
  });
}
