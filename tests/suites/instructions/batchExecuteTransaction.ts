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
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  range,
  TestMembers,
  formatsToRun,
  getRpc,
} from "../../utils";

const { Proposal } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

for (const format of formatsToRun) {
  const rpc = getRpc(format);

  describe(`Instructions / batch_execute_transaction [${format}]`, () => {
    let members: TestMembers;

    before(async () => {
      members = await generateSmartAccountSigners(connection);
    });

    it("execute all transactions in a batch", async () => {
      const feePayer = await generateFundedKeypair(connection);
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

      // Airdrop enough for 3 transfers
      await connection.confirmTransaction(
        await connection.requestAirdrop(vaultPda, 5 * LAMPORTS_PER_SOL)
      );

      const batchIndex = 1n;
      const numTransactions = 3;
      const payees: Keypair[] = [];

      // Create batch
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

      // Create draft proposal
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

      // Add transactions to the batch
      const blockhash = (await connection.getLatestBlockhash()).blockhash;
      for (let i = 1; i <= numTransactions; i++) {
        const payee = Keypair.generate();
        payees.push(payee);

        const testIx = createTestTransferInstruction(
          vaultPda,
          payee.publicKey,
          LAMPORTS_PER_SOL
        );
        const testTransferMessage = new TransactionMessage({
          payerKey: vaultPda,
          recentBlockhash: blockhash,
          instructions: [testIx],
        });

        signature = await rpc.addTransactionToBatch({
          connection,
          feePayer: members.proposer,
          settingsPda,
          signer: members.proposer,
          accountIndex: 0,
          batchIndex,
          transactionIndex: i,
          ephemeralSigners: 0,
          transactionMessage: testTransferMessage,
          addressLookupTableAccounts: [],
          programId,
        });
        await connection.confirmTransaction(signature);
      }

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

      // First approval
      signature = await rpc.approveProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        signer: members.voter,
        transactionIndex: batchIndex,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Second approval
      signature = await rpc.approveProposal({
        connection,
        feePayer: members.almighty,
        settingsPda,
        signer: members.almighty,
        transactionIndex: batchIndex,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Execute all batch transactions
      for (const txIndex of range(1, numTransactions)) {
        signature = await rpc.executeBatchTransaction({
          connection,
          feePayer,
          settingsPda,
          signer: members.executor,
          batchIndex,
          transactionIndex: txIndex,
          programId,
        });
        await connection.confirmTransaction(signature);
      }

      // Verify proposal status is Executed
      const [proposalPda] = smartAccount.getProposalPda({
        settingsPda,
        transactionIndex: batchIndex,
        programId,
      });
      const proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );
      assert.ok(
        smartAccount.types.isProposalStatusExecuted(proposalAccount.status)
      );

      // Verify each payee received their SOL
      for (const payee of payees) {
        const balance = await connection.getBalance(payee.publicKey);
        assert.strictEqual(balance, LAMPORTS_PER_SOL);
      }
    });
  });
}
