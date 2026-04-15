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

describe("Instructions / authority_remove_signer", () => {
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
      smartAccount.rpc.removeSignerAsAuthority({
        connection,
        feePayer,
        settingsPda: settingsPda,
        settingsAuthority: wrongConfigAuthority.publicKey,
        oldSigner: members.proposer.publicKey,
        programId,
        signers: [wrongConfigAuthority],
      }),
      /Attempted to perform an unauthorized action/
    );
  });

  it("remove the signer for the controlled smart account", async () => {
    const signature = await smartAccount.rpc.removeSignerAsAuthority({
      connection,
      feePayer: members.proposer,
      settingsPda: settingsPda,
      settingsAuthority: configAuthority.publicKey,
      oldSigner: members.voter.publicKey,
      programId,
      signers: [configAuthority],
    });
    await connection.confirmTransaction(signature);
  });
});
