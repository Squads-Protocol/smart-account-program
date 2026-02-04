import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import assert from "assert";
import {
  createAutonomousSmartAccountV2,
  createLocalhostConnection,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const { Settings } = smartAccount.accounts;
const programId = getTestProgramId();
const connection = createLocalhostConnection();

function createIncrementAccountIndexInstruction(
  settingsPda: PublicKey,
  signer: PublicKey,
  programId: PublicKey
) {
  return smartAccount.generated.createIncrementAccountIndexInstruction(
    { settings: settingsPda, signer, program: programId },
    programId
  );
}

describe("Instructions / increment_account_index", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("increment_account_index successfully", async () => {
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

    // Check initial account_utilization is 0
    let settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settingsAccount.accountUtilization, 0);

    // Increment the account index
    const incrementIx = createIncrementAccountIndexInstruction(
      settingsPda,
      members.almighty.publicKey,
      programId
    );

    const message = new TransactionMessage({
      payerKey: members.almighty.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [incrementIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([members.almighty]);

    const signature = await connection.sendRawTransaction(tx.serialize());
    await connection.confirmTransaction(signature);

    // Verify account_utilization is now 1
    settingsAccount = await Settings.fromAccountAddress(connection, settingsPda);
    assert.strictEqual(settingsAccount.accountUtilization, 1);
  });

  it("increment multiple times", async () => {
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

    // Increment 3 times
    for (let i = 0; i < 3; i++) {
      const incrementIx = createIncrementAccountIndexInstruction(
        settingsPda,
        members.almighty.publicKey,
        programId
      );

      const message = new TransactionMessage({
        payerKey: members.almighty.publicKey,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions: [incrementIx],
      }).compileToV0Message();

      const tx = new VersionedTransaction(message);
      tx.sign([members.almighty]);

      await connection.confirmTransaction(
        await connection.sendRawTransaction(tx.serialize())
      );
    }

    const settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settingsAccount.accountUtilization, 3);
  });

  it("error: non-signer cannot increment", async () => {
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

    // Create a random keypair that is not a signer on the smart account
    const nonSigner = Keypair.generate();
    await connection.confirmTransaction(
      await connection.requestAirdrop(nonSigner.publicKey, LAMPORTS_PER_SOL)
    );

    const incrementIx = createIncrementAccountIndexInstruction(
      settingsPda,
      nonSigner.publicKey,
      programId
    );

    const message = new TransactionMessage({
      payerKey: nonSigner.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [incrementIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([nonSigner]);

    await assert.rejects(
      async () => {
        await connection
          .sendRawTransaction(tx.serialize())
          .catch(smartAccount.errors.translateAndThrowAnchorError);
      },
      /NotASigner/
    );
  });

  it("proposer can increment (has Initiate permission)", async () => {
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

    // Proposer only has Initiate permission
    const incrementIx = createIncrementAccountIndexInstruction(
      settingsPda,
      members.proposer.publicKey,
      programId
    );

    const message = new TransactionMessage({
      payerKey: members.proposer.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [incrementIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([members.proposer]);

    const signature = await connection.sendRawTransaction(tx.serialize());
    await connection.confirmTransaction(signature);

    const settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settingsAccount.accountUtilization, 1);
  });

  it("voter can increment (has Vote permission)", async () => {
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

    // Voter only has Vote permission
    const incrementIx = createIncrementAccountIndexInstruction(
      settingsPda,
      members.voter.publicKey,
      programId
    );

    const message = new TransactionMessage({
      payerKey: members.voter.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [incrementIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([members.voter]);

    const signature = await connection.sendRawTransaction(tx.serialize());
    await connection.confirmTransaction(signature);

    const settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settingsAccount.accountUtilization, 1);
  });

  it("executor can increment (has Execute permission)", async () => {
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

    // Executor only has Execute permission
    const incrementIx = createIncrementAccountIndexInstruction(
      settingsPda,
      members.executor.publicKey,
      programId
    );

    const message = new TransactionMessage({
      payerKey: members.executor.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [incrementIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([members.executor]);

    const signature = await connection.sendRawTransaction(tx.serialize());
    await connection.confirmTransaction(signature);

    const settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settingsAccount.accountUtilization, 1);
  });

  it("error: cannot increment beyond max index (250)", async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = await createAutonomousSmartAccountV2({
      connection,
      accountIndex,
      members,
      threshold: 1,
      timeLock: 0,
      rentCollector: null,
      programId,
    });

    // Batch increment instructions to reach 250 efficiently
    // ~20 instructions per tx to stay within limits
    // Note: yes this is stupid but we can't mock the account_utilization
    // to be at 250 already so we need to batch send.
    const BATCH_SIZE = 20;
    const TARGET = 250;

    for (let i = 0; i < TARGET; i += BATCH_SIZE) {
      const count = Math.min(BATCH_SIZE, TARGET - i);
      const instructions = Array.from({ length: count }, () =>
        createIncrementAccountIndexInstruction(
          settingsPda,
          members.almighty.publicKey,
          programId
        )
      );

      const message = new TransactionMessage({
        payerKey: members.almighty.publicKey,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions,
      }).compileToV0Message();

      const tx = new VersionedTransaction(message);
      tx.sign([members.almighty]);
      await connection.confirmTransaction(
        await connection.sendRawTransaction(tx.serialize())
      );
    }

    // Verify we're at 250
    let settings = await Settings.fromAccountAddress(connection, settingsPda);
    assert.strictEqual(settings.accountUtilization, 250);

    // Now try to increment to 251 - should fail
    const incrementIx = createIncrementAccountIndexInstruction(
      settingsPda,
      members.almighty.publicKey,
      programId
    );

    const message = new TransactionMessage({
      payerKey: members.almighty.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [incrementIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([members.almighty]);

    await assert.rejects(
      async () => {
        await connection
          .sendRawTransaction(tx.serialize())
          .catch(smartAccount.errors.translateAndThrowAnchorError);
      },
      /MaxAccountIndexReached/
    );
  });
});
