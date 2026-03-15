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
  createSignerArray,
  isCloseToNow,
  formatsToRun,
  getRpc,
} from "../../utils";

const { toBigInt } = smartAccount.utils;
const { Settings, Proposal } = smartAccount.accounts;
const { Permission, Permissions } = smartAccount.types;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

for (const format of formatsToRun) {
  const rpc = getRpc(format);

  describe(`Instructions / proposal_create [${format}]`, () => {
    let members: TestMembers;

    before(async () => {
      members = await generateSmartAccountSigners(connection);
    });

    describe("proposal_create", () => {
      let settingsPda: PublicKey;

      before(async () => {
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
        const newSignerKey = Keypair.generate().publicKey;

        let signature = await rpc.createSettingsTransaction({
          connection,
          feePayer: members.proposer,
          settingsPda,
          transactionIndex: 1n,
          creator: members.proposer.publicKey,
          actions: [
            {
              __kind: "AddSigner",
              newSigner: createSignerArray(newSignerKey, Permissions.all()),
            },
          ],
          programId,
        });
        await connection.confirmTransaction(signature);
      });

      it("error: invalid transaction index", async () => {
        // Attempt to create a proposal for a transaction that doesn't exist.
        const transactionIndex = 2n;
        await assert.rejects(
          () =>
            rpc.createProposal({
              connection,
              feePayer: members.almighty,
              settingsPda,
              transactionIndex,
              creator: members.almighty,
              programId,
            }),
          /Invalid transaction index/
        );
      });

      it("error: non-signers can't create a proposal", async () => {
        const nonMember = await generateFundedKeypair(connection);

        const transactionIndex = 2n;

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

        await assert.rejects(
          () =>
            rpc.createProposal({
              connection,
              feePayer: nonMember,
              settingsPda,
              transactionIndex,
              creator: nonMember,
              programId,
            }),
          /Provided pubkey is not a signer of the smart account/
        );
      });

      it("error: signers without Initiate or Vote permissions can't create a proposal", async () => {
        const transactionIndex = 2n;

        await assert.rejects(
          () =>
            rpc.createProposal({
              connection,
              feePayer: members.executor,
              settingsPda,
              transactionIndex,
              creator: members.executor,
              programId,
            }),
          /Attempted to perform an unauthorized action/
        );
      });

      it("signer with Initiate or Vote permissions can create proposal", async () => {
        const nonMember = await generateFundedKeypair(connection);

        const transactionIndex = 2n;

        // Create a proposal for the settings transaction.
        let signature = await rpc.createProposal({
          connection,
          feePayer: members.voter,
          settingsPda,
          transactionIndex,
          creator: members.voter,
          programId,
        });
        await connection.confirmTransaction(signature);

        // Fetch the newly created Proposal account.
        const [proposalPda, proposalBump] = smartAccount.getProposalPda({
          settingsPda,
          transactionIndex,
          programId,
        });
        const proposalAccount = await Proposal.fromAccountAddress(
          connection,
          proposalPda
        );

        // Make sure the proposal was created correctly.
        assert.strictEqual(
          proposalAccount.settings.toBase58(),
          settingsPda.toBase58()
        );
        assert.strictEqual(
          proposalAccount.transactionIndex.toString(),
          transactionIndex.toString()
        );
        assert.ok(
          smartAccount.types.isProposalStatusActive(proposalAccount.status)
        );
        assert.ok(isCloseToNow(toBigInt(proposalAccount.status.timestamp)));
        assert.strictEqual(proposalAccount.bump, proposalBump);
        assert.deepEqual(proposalAccount.approved, []);
        assert.deepEqual(proposalAccount.rejected, []);
        assert.deepEqual(proposalAccount.cancelled, []);
      });

      it("error: cannot create proposal for stale transaction", async () => {
        // Approve the second settings transaction.
        let signature = await rpc.approveProposal({
          connection,
          feePayer: members.voter,
          settingsPda,
          transactionIndex: 2n,
          signer: members.voter,
          programId,
        });
        await connection.confirmTransaction(signature);

        signature = await rpc.approveProposal({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionIndex: 2n,
          signer: members.almighty,
          programId,
        });
        await connection.confirmTransaction(signature);

        // Execute the second settings transaction.
        signature = await rpc.executeSettingsTransaction({
          connection,
          feePayer: members.almighty,
          settingsPda,
          transactionIndex: 2n,
          signer: members.almighty,
          rentPayer: members.almighty,
          programId,
        });
        await connection.confirmTransaction(signature);

        const feePayer = await generateFundedKeypair(connection);

        // At this point the first transaction should become stale.
        // Attempt to create a proposal for it should fail.
        await assert.rejects(
          () =>
            rpc.createProposal({
              connection,
              feePayer,
              settingsPda,
              transactionIndex: 1n,
              creator: members.almighty,
              programId,
            }),
          /Proposal is stale/
        );
      });
    });
  });
}
