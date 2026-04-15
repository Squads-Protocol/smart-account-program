import {
  Keypair,
  LAMPORTS_PER_SOL,
  TransactionMessage,
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

  describe(`Instructions / transaction_close [${format}]`, () => {
    let members: TestMembers;

    before(async () => {
      members = await generateSmartAccountSigners(connection);
    });

    it("close an executed transaction and reclaim rent", async () => {
      const accountIndex = await getNextAccountIndex(connection, programId);

      const [vaultPda] = smartAccount.getSmartAccountPda({
        settingsPda: smartAccount.getSettingsPda({ accountIndex, programId })[0],
        accountIndex: 0,
        programId,
      });

      const [settingsPda] = await createAutonomousSmartAccountV2({
        connection,
        accountIndex,
        members,
        threshold: 1,
        timeLock: 0,
        rentCollector: vaultPda,
        programId,
      });

      // Airdrop to vault
      await connection.confirmTransaction(
        await connection.requestAirdrop(vaultPda, 2 * LAMPORTS_PER_SOL)
      );

      // Create transfer instruction
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

      const transactionIndex = 1n;

      // Create transaction
      let signature = await rpc.createTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        accountIndex: 0,
        transactionMessage: testTransferMessage,
        ephemeralSigners: 0,
        creator: members.proposer.publicKey,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Create proposal
      signature = await rpc.createProposal({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        creator: members.proposer,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Approve
      signature = await rpc.approveProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex,
        signer: members.voter,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Execute
      signature = await rpc.executeTransaction({
        connection,
        feePayer: members.executor,
        settingsPda,
        transactionIndex,
        signer: members.executor.publicKey,
        signers: [members.executor],
        programId,
      });
      await connection.confirmTransaction(signature);

      // Verify executed
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

      // Record pre-close balance
      const preBalance = await connection.getBalance(
        members.proposer.publicKey
      );

      // Close transaction accounts
      signature = await rpc.closeTransaction({
        connection,
        feePayer: members.almighty,
        settingsPda,
        transactionRentCollector: members.proposer.publicKey,
        proposalRentCollector: members.proposer.publicKey,
        transactionIndex,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Verify rent was returned
      const postBalance = await connection.getBalance(
        members.proposer.publicKey
      );
      assert.ok(postBalance > preBalance);

      // Verify accounts are closed
      const transactionPda = smartAccount.getTransactionPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0];
      assert.strictEqual(
        await connection.getAccountInfo(transactionPda),
        null
      );
      assert.strictEqual(
        await connection.getAccountInfo(proposalPda),
        null
      );
    });
  });
}
