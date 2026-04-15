import { Keypair, PublicKey } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import * as assert from "assert";
import {
  createAutonomousMultisig,
  createControlledSmartAccount,
  createLocalhostConnection,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / settings_transaction_set_time_lock", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  let settingsPda: PublicKey;
  let configAuthority: Keypair;
  before(async () => {
    configAuthority = await generateFundedKeypair(connection);
    const accountIndex = await getNextAccountIndex(connection, programId);
    // Create new controlled smartAccount.
    settingsPda = (
      await createAutonomousMultisig({
        connection,
        accountIndex,
        members,
        threshold: 1,
        timeLock: 0,
        programId,
      })
    )[0];
  });
  it("error: invalid authority", async () => {
    const feePayer = await generateFundedKeypair(connection);
    await assert.rejects(
      smartAccount.rpc.createSettingsTransaction({
        connection,
        feePayer,
        settingsPda: settingsPda,
        transactionIndex: 1n,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "SetTimeLock", newTimeLock: 300 }],
        programId,
      })
    ),
      /Attempted to perform an unauthorized action/;
  });

  it("set `time_lock` for the autonomous smart account", async () => {
    const signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda: settingsPda,
      transactionIndex: 1n,
      creator: members.proposer.publicKey,
      actions: [{ __kind: "SetTimeLock", newTimeLock: 300 }],
      programId,
    });
    await connection.confirmTransaction(signature);
  });
});

describe("Instructions / set_time_lock_as_authority", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  let settingsPda: PublicKey;
  let configAuthority: Keypair;
  let wrongConfigAuthority: Keypair;
  before(async () => {
    configAuthority = await generateFundedKeypair(connection);
    wrongConfigAuthority = await generateFundedKeypair(connection);
    const accountIndex = await getNextAccountIndex(connection, programId);
    // Create new controlled smartAccount.
    settingsPda = (
      await createControlledSmartAccount({
        connection,
        accountIndex,
        members,
        threshold: 1,
        configAuthority: configAuthority.publicKey,
        timeLock: 0,
        programId,
      })
    )[0];
  });
  it("error: invalid authority", async () => {
    const feePayer = await generateFundedKeypair(connection);
    await assert.rejects(
      smartAccount.rpc.setTimeLockAsAuthority({
        connection,
        feePayer,
        settingsPda: settingsPda,
        settingsAuthority: wrongConfigAuthority.publicKey,
        timeLock: 300,
        signers: [feePayer, wrongConfigAuthority],
        programId,
      })
    ),
      /Attempted to perform an unauthorized action/;
  });

  it("set `time_lock` for the controlled smart account", async () => {
    const feePayer = await generateFundedKeypair(connection);
    const signature = await smartAccount.rpc.setTimeLockAsAuthority({
      connection,
      feePayer,
      settingsPda: settingsPda,
      settingsAuthority: configAuthority.publicKey,
      timeLock: 300,
      signers: [feePayer, configAuthority],
      programId,
    });
    await connection.confirmTransaction(signature);
  });
});
