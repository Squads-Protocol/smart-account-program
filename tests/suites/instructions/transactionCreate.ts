import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  TransactionMessage,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import * as assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  createTestTransferInstruction,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
  formatsToRun,
  getRpc,
} from "../../utils";

const { Settings } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

for (const format of formatsToRun) {
  const rpc = getRpc(format);

  describe(`Instructions / transaction_create [${format}]`, () => {
    let members: TestMembers;
    let settingsPda: PublicKey;

    before(async () => {
      members = await generateSmartAccountSigners(connection);
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
    });

    it("error: not a signer", async () => {
      const nonMember = await generateFundedKeypair(connection);

      // Default vault.
      const [vaultPda] = smartAccount.getSmartAccountPda({
        settingsPda,
        accountIndex: 0,
        programId,
      });

      // Test transfer instruction.
      const testPayee = Keypair.generate();
      const testIx = await createTestTransferInstruction(
        vaultPda,
        testPayee.publicKey
      );
      const testTransferMessage = new TransactionMessage({
        payerKey: vaultPda,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions: [testIx],
      });

      await assert.rejects(
        () =>
          rpc.createTransaction({
            connection,
            feePayer: nonMember,
            settingsPda,
            transactionIndex: 1n,
            creator: nonMember.publicKey,
            accountIndex: 0,
            ephemeralSigners: 0,
            transactionMessage: testTransferMessage,
            programId,
          }),
        /Provided pubkey is not a signer of the smart account/
      );
    });

    it("error: unauthorized", async () => {
      // Default vault.
      const [vaultPda] = smartAccount.getSmartAccountPda({
        settingsPda,
        accountIndex: 0,
        programId,
      });

      // Test transfer instruction.
      const testPayee = Keypair.generate();
      const testIx = await createTestTransferInstruction(
        vaultPda,
        testPayee.publicKey
      );
      const testTransferMessage = new TransactionMessage({
        payerKey: vaultPda,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions: [testIx],
      });

      await assert.rejects(
        () =>
          rpc.createTransaction({
            connection,
            feePayer: members.voter,
            settingsPda,
            transactionIndex: 1n,
            creator: members.voter.publicKey,
            accountIndex: 0,
            ephemeralSigners: 0,
            transactionMessage: testTransferMessage,
            programId,
          }),
        /Attempted to perform an unauthorized action/
      );
    });

    it("create a new transaction", async () => {
      const transactionIndex = 1n;

      // Default vault.
      const [vaultPda, vaultBump] = smartAccount.getSmartAccountPda({
        settingsPda,
        accountIndex: 0,
        programId,
      });

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

      const signature = await rpc.createTransaction({
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

      const multisigAccount = await Settings.fromAccountAddress(
        connection,
        settingsPda
      );
      assert.strictEqual(
        multisigAccount.transactionIndex.toString(),
        transactionIndex.toString()
      );

      // Verify the transaction PDA exists on-chain
      const [transactionPda] = smartAccount.getTransactionPda({
        settingsPda,
        transactionIndex,
        programId,
      });
      const transactionAccountInfo = await connection.getAccountInfo(transactionPda);
      assert.ok(transactionAccountInfo, "Transaction account should exist");
      assert.ok(transactionAccountInfo.data.length > 0, "Transaction account should have data");
    });
  });
}
