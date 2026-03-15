import { Keypair, PublicKey } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import assert from "assert";
import {
  createControlledSmartAccount,
  createLocalhostConnection,
  createSignerObject,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  getSignerKey,
  TestMembers,
} from "../../utils";

const { Settings } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / authority_settings_transaction_execute", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("authority adds and removes a signer on a controlled smart account", async () => {
    const configAuthority = await generateFundedKeypair(connection);
    const feePayer = await generateFundedKeypair(connection);
    const accountIndex = await getNextAccountIndex(connection, programId);

    const [settingsPda] = await createControlledSmartAccount({
      connection,
      accountIndex,
      configAuthority: configAuthority.publicKey,
      members,
      threshold: 2,
      timeLock: 0,
      programId,
    });

    // Verify initial threshold
    let multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(multisigAccount.threshold, 2);

    // Add a signer as authority
    const newSignerKey = Keypair.generate().publicKey;
    const newSigner = createSignerObject(
      newSignerKey,
      smartAccount.types.Permissions.all()
    );

    const addSignerSig = await smartAccount.rpc.addSignerAsAuthority({
      connection,
      feePayer,
      settingsPda,
      settingsAuthority: configAuthority.publicKey,
      rentPayer: feePayer,
      newSigner,
      signers: [configAuthority],
      programId,
    });
    await connection.confirmTransaction(addSignerSig);

    // Verify signer was added
    multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    const signerKeys = multisigAccount.signers.map((s) => getSignerKey(s));
    assert.ok(
      signerKeys.some((k) => k.equals(newSignerKey)),
      "New signer should be in the signers list"
    );

    // Authority removes the newly added signer
    const removeSignerSig = await smartAccount.rpc.removeSignerAsAuthority({
      connection,
      feePayer,
      settingsPda,
      settingsAuthority: configAuthority.publicKey,
      oldSigner: newSignerKey,
      signers: [configAuthority],
      programId,
    });
    await connection.confirmTransaction(removeSignerSig);

    // Verify signer was removed
    multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    const updatedSignerKeys = multisigAccount.signers.map((s) =>
      getSignerKey(s)
    );
    assert.ok(
      !updatedSignerKeys.some((k) => k.equals(newSignerKey)),
      "New signer should have been removed"
    );
  });
});
