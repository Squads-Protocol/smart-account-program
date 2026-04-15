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

  describe(`Instructions / batch_add_transaction [${format}]`, () => {
    let members: TestMembers;

    before(async () => {
      members = await generateSmartAccountSigners(connection);
    });

    it("add multiple transactions to a batch", async () => {
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

      // Airdrop to vault
      await connection.confirmTransaction(
        await connection.requestAirdrop(vaultPda, 5 * LAMPORTS_PER_SOL)
      );

      const batchIndex = 1n;

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

      // Add 3 transactions to the batch
      const blockhash = (await connection.getLatestBlockhash()).blockhash;
      for (let i = 1; i <= 3; i++) {
        const testPayee = Keypair.generate();
        const testIx = createTestTransferInstruction(
          vaultPda,
          testPayee.publicKey,
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

      // Verify batch transactions were created by checking their PDAs exist
      for (let i = 1; i <= 3; i++) {
        const [batchTxPda] = smartAccount.getBatchTransactionPda({
          settingsPda,
          batchIndex,
          transactionIndex: i,
          programId,
        });
        const txAccount = await connection.getAccountInfo(batchTxPda);
        assert.ok(txAccount !== null, `Batch transaction ${i} should exist`);
      }

      // Activate the proposal to confirm the batch is finalized
      signature = await rpc.activateProposal({
        connection,
        feePayer: members.proposer,
        settingsPda,
        signer: members.proposer,
        transactionIndex: batchIndex,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Verify proposal is Active
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
        smartAccount.types.isProposalStatusActive(proposalAccount.status)
      );
    });
  });
}
