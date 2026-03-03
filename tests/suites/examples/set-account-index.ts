import {
  Keypair,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import assert from "assert";
import { readFileSync } from "fs";
import path from "path";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const programId = getTestProgramId();
const { Settings } = smartAccount.accounts;

function getTestPaymaster(): Keypair {
  return Keypair.fromSecretKey(
    Buffer.from(
      JSON.parse(
        readFileSync(
          path.join(__dirname, "../../fixtures/paymaster-test.json"),
          "utf-8"
        )
      )
    )
  );
}

describe("Examples / Set Account Index", () => {
  const connection = createLocalhostConnection();

  let members: TestMembers;
  let paymaster: Keypair;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
    paymaster = getTestPaymaster();

    // Fund the paymaster
    const sig = await connection.requestAirdrop(
      paymaster.publicKey,
      1_000_000_000
    );
    await connection.confirmTransaction(sig);
  });

  it("paymaster can set account index", async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);

    const [settingsPda] = await createAutonomousMultisig({
      connection,
      members,
      threshold: 1,
      timeLock: 0,
      programId,
      accountIndex,
    });

    // Verify initial account_utilization is 0
    let settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settingsAccount.accountUtilization, 0);

    // Set account index to 5
    const ix = smartAccount.generated.createSetAccountIndexInstruction(
      {
        settings: settingsPda,
        paymaster: paymaster.publicKey,
        program: programId,
      },
      {
        args: { newIndex: 5 },
      },
      programId
    );

    const message = new TransactionMessage({
      payerKey: paymaster.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [ix],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([paymaster]);

    const signature = await connection.sendRawTransaction(tx.serialize(), {
      skipPreflight: true,
    });
    await connection.confirmTransaction(signature);

    // Verify account_utilization is now 5
    settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settingsAccount.accountUtilization, 5);
  });

  it("non-paymaster cannot set account index", async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);

    const [settingsPda] = await createAutonomousMultisig({
      connection,
      members,
      threshold: 1,
      timeLock: 0,
      programId,
      accountIndex,
    });

    // Try to set account index with a random keypair (not the paymaster)
    const fakeSigner = members.almighty;

    const ix = smartAccount.generated.createSetAccountIndexInstruction(
      {
        settings: settingsPda,
        paymaster: fakeSigner.publicKey,
        program: programId,
      },
      {
        args: { newIndex: 5 },
      },
      programId
    );

    const message = new TransactionMessage({
      payerKey: fakeSigner.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [ix],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([fakeSigner]);

    await assert.rejects(
      () =>
        connection
          .sendRawTransaction(tx.serialize())
          .catch(smartAccount.errors.translateAndThrowAnchorError),
      /Unauthorized/
    );

    // Verify account_utilization is still 0
    const settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settingsAccount.accountUtilization, 0);
  });
});
