import {
  Keypair,
  LAMPORTS_PER_SOL,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import assert from "assert";
import {
  createAutonomousSmartAccountV2,
  createLocalhostConnection,
  createTestTransferInstruction,
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

  describe(`Instructions / activate_proposal [${format}]`, () => {
    let members: TestMembers;

    before(async () => {
      members = await generateSmartAccountSigners(connection);
    });

    it("activate a draft proposal for a batch", async () => {
      const accountIndex = await getNextAccountIndex(connection, programId);
      const [settingsPda] = await createAutonomousSmartAccountV2({
        connection,
        accountIndex,
        members,
        threshold: 2,
        timeLock: 0,
        rentCollector: null,
        programId,
      });

      const [vaultPda] = smartAccount.getSmartAccountPda({
        settingsPda,
        accountIndex: 0,
        programId,
      });

      // Airdrop to vault for transfer instructions
      await connection.confirmTransaction(
        await connection.requestAirdrop(vaultPda, 2 * LAMPORTS_PER_SOL)
      );

      const batchIndex = 1n;

      // Create a batch
      let signature = await rpc.createBatch({
        connection,
        feePayer: members.proposer,
        settingsPda,
        creator: members.proposer,
        batchIndex,
        accountIndex: 0,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Create a draft proposal for the batch
      signature = await rpc.createProposal({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex: batchIndex,
        creator: members.proposer,
        isDraft: true,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Verify the proposal is in Draft status
      const [proposalPda] = smartAccount.getProposalPda({
        settingsPda,
        transactionIndex: batchIndex,
        programId,
      });
      let proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );
      assert.strictEqual(proposalAccount.status.__kind, "Draft");

      // Add a transaction to the batch
      const testPayee = Keypair.generate();
      const testIx = createTestTransferInstruction(
        vaultPda,
        testPayee.publicKey,
        LAMPORTS_PER_SOL
      );
      const testTransferMessage = new TransactionMessage({
        payerKey: vaultPda,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions: [testIx],
      });

      signature = await rpc.addTransactionToBatch({
        connection,
        feePayer: members.proposer,
        settingsPda,
        signer: members.proposer,
        accountIndex: 0,
        batchIndex,
        transactionIndex: 1,
        ephemeralSigners: 0,
        transactionMessage: testTransferMessage,
        addressLookupTableAccounts: [],
        programId,
      });
      await connection.confirmTransaction(signature);

      // Activate the proposal
      signature = await rpc.activateProposal({
        connection,
        feePayer: members.proposer,
        settingsPda,
        signer: members.proposer,
        transactionIndex: batchIndex,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Verify the proposal is now Active
      proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );
      assert.ok(
        smartAccount.types.isProposalStatusActive(proposalAccount.status)
      );
    });
  });
}
