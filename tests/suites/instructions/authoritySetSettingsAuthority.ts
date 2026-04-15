import { Keypair, PublicKey } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import * as assert from "assert";
import {
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

describe("Instructions / set_settings_authority_as_authority (with error)", () => {
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
      await createControlledSmartAccount({
        connection,
        accountIndex,
        members,
        threshold: 1,
        timeLock: 0,
        configAuthority: configAuthority.publicKey,
        programId,
      })
    )[0];
  });

  it("error: invalid authority", async () => {
    const feePayer = await generateFundedKeypair(connection);
    await assert.rejects(
      smartAccount.rpc.setNewSettingsAuthorityAsAuthority({
        connection,
        feePayer,
        settingsPda: settingsPda,
        settingsAuthority: members.voter.publicKey,
        newSettingsAuthority: members.voter.publicKey,
        programId,
      })
    ),
      /Attempted to perform an unauthorized action/;
  });

  it("set `settings authority` for the controlled smart account", async () => {
    const feePayer = await generateFundedKeypair(connection);
    const signature =
      await smartAccount.rpc.setNewSettingsAuthorityAsAuthority({
        connection,
        feePayer,
        settingsPda: settingsPda,
        settingsAuthority: configAuthority.publicKey,
        newSettingsAuthority: members.voter.publicKey,
        signers: [feePayer, configAuthority],
        programId,
      });
    await connection.confirmTransaction(signature);
  });
});

describe("Instructions / set_settings_authority_as_authority (create controlled)", () => {
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
      await createControlledSmartAccount({
        connection,
        accountIndex,
        configAuthority: configAuthority.publicKey,
        members,
        threshold: 1,
        timeLock: 0,
        programId,
      })
    )[0];
  });

  it("set `settings_authority` for the controlled smart account", async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);
    await createControlledSmartAccount({
      accountIndex,
      configAuthority: members.almighty.publicKey,
      members,
      connection,
      threshold: 2,
      timeLock: 0,
      programId,
    });
  });
});
