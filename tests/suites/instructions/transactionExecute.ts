import {
  Keypair,
  LAMPORTS_PER_SOL,
  TransactionMessage,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import * as assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  createTestTransferInstruction,
  generateSmartAccountSigners,
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

  describe(`Instructions / transaction_execute [${format}]`, () => {
    let members: TestMembers;

    before(async () => {
      members = await generateSmartAccountSigners(connection);
    });

    it("execute a transaction", async () => {
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

      // Default vault.
      const [vaultPda] = smartAccount.getSmartAccountPda({
        settingsPda,
        accountIndex: 0,
        programId,
      });

      // Airdrop 2 SOL to the Vault.
      const airdropSig = await connection.requestAirdrop(
        vaultPda,
        2 * LAMPORTS_PER_SOL
      );
      await connection.confirmTransaction(airdropSig);

      // Test transfer instruction (2x)
      const testPayee = Keypair.generate();
      const testIx1 = await createTestTransferInstruction(
        vaultPda,
        testPayee.publicKey,
        1 * LAMPORTS_PER_SOL
      );
      const testIx2 = await createTestTransferInstruction(
        vaultPda,
        testPayee.publicKey,
        1 * LAMPORTS_PER_SOL
      );
      const testTransferMessage = new TransactionMessage({
        payerKey: vaultPda,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions: [testIx1, testIx2],
      });

      const transactionIndex = 1n;

      // Create a transaction.
      let signature = await rpc.createTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        creator: members.proposer.publicKey,
        accountIndex: 0,
        ephemeralSigners: 0,
        transactionMessage: testTransferMessage,
        memo: "Transfer 2 SOL to a test account",
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

      // Approve the proposal by the first member.
      signature = await rpc.approveProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex,
        signer: members.voter,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Approve the proposal by the second member.
      signature = await rpc.approveProposal({
        connection,
        feePayer: members.almighty,
        settingsPda,
        transactionIndex,
        signer: members.almighty,
        programId,
      });
      await connection.confirmTransaction(signature);

      const [proposalPda] = smartAccount.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      });
      const preVaultBalance = await connection.getBalance(vaultPda);
      assert.strictEqual(preVaultBalance, 2 * LAMPORTS_PER_SOL);

      // Execute the transaction.
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

      // Verify the transaction account.
      const proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );
      assert.ok(
        smartAccount.types.isProposalStatusExecuted(proposalAccount.status)
      );

      const postVaultBalance = await connection.getBalance(vaultPda);
      // Transferred 2 SOL to payee.
      assert.strictEqual(postVaultBalance, 0);
    });

    it("error: not a member");

    it("error: unauthorized");

    it("error: invalid transaction status");

    it("error: transaction is not for smart account");

    it("error: execute reentrancy");
  });
}
