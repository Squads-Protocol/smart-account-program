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
  unwrapSigners,
  createSignerObject,
} from "../../utils";

const { Settings } = smartAccount.accounts;
const { Permissions } = smartAccount.types;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / authority_add_signer", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  const newSigner: smartAccount.types.SmartAccountSigner = {
    __kind: "Native",
    key: Keypair.generate().publicKey,
    permissions: Permissions.all(),
  };
  const newMember2: smartAccount.types.SmartAccountSigner = {
    __kind: "Native",
    key: Keypair.generate().publicKey,
    permissions: Permissions.all(),
  };

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
        threshold: 2,
        timeLock: 0,
        programId,
      })
    )[0];
  });

  it("error: adding an existing member", async () => {
    const feePayer = await generateFundedKeypair(connection);

    // Adding the samesigneragain should fail.
    await assert.rejects(
      smartAccount.rpc.addSignerAsAuthority({
        connection,
        feePayer,
        settingsPda,
        settingsAuthority: configAuthority.publicKey,
        rentPayer: configAuthority,
        newSigner: createSignerObject(
          members.almighty.publicKey,
          Permissions.all()
        ),
        signers: [configAuthority],
        programId,
      }),
      /Found multiple signers with the same pubkey/
    );
  });

  it("error: missing authority signature", async () => {
    const feePayer = await generateFundedKeypair(connection);

    await assert.rejects(
      smartAccount.rpc.addSignerAsAuthority({
        connection,
        feePayer,
        settingsPda,
        settingsAuthority: configAuthority.publicKey,
        rentPayer: feePayer,
        newSigner,
        signers: [
          /* missing authority signature */
        ],
        programId,
      }),
      /Transaction signature verification failure/
    );
  });

  it("error: invalid authority", async () => {
    const fakeAuthority = await generateFundedKeypair(connection);

    await assert.rejects(
      smartAccount.rpc.addSignerAsAuthority({
        connection,
        feePayer: fakeAuthority,
        settingsPda,
        settingsAuthority: fakeAuthority.publicKey,
        rentPayer: fakeAuthority,
        newSigner,
        signers: [fakeAuthority],
        programId,
      }),
      /Attempted to perform an unauthorized action/
    );
  });

  it("add a new signer to the controlled smart account", async () => {
    // feePayer can be anyone.
    const feePayer = await generateFundedKeypair(connection);

    let multisigAccountInfo = await connection.getAccountInfo(settingsPda);
    assert.ok(multisigAccountInfo);
    let [multisigAccount] = Settings.fromAccountInfo(multisigAccountInfo);

    const initialMembersLength = unwrapSigners(multisigAccount.signers).length;
    const initialOccupiedSize =
      smartAccount.generated.settingsBeet.toFixedFromValue({
        accountDiscriminator: smartAccount.generated.settingsDiscriminator,
        ...multisigAccount,
      }).byteSize;
    const initialAllocatedSize = multisigAccountInfo.data.length;

    // Right after the creation of the smart account, the allocated account space is fully utilized,
    assert.equal(initialOccupiedSize, initialAllocatedSize);

    let signature = await smartAccount.rpc.addSignerAsAuthority({
      connection,
      feePayer,
      settingsPda,
      settingsAuthority: configAuthority.publicKey,
      rentPayer: configAuthority,
      newSigner,
      memo: "Adding my good friend to the smart account",
      signers: [configAuthority],
      sendOptions: { skipPreflight: true },
      programId,
    });
    await connection.confirmTransaction(signature);

    multisigAccountInfo = await connection.getAccountInfo(settingsPda);
    multisigAccount = Settings.fromAccountInfo(multisigAccountInfo!)[0];

    let newMembersLength = unwrapSigners(multisigAccount.signers).length;
    let newOccupiedSize =
      smartAccount.generated.settingsBeet.toFixedFromValue({
        accountDiscriminator: smartAccount.generated.settingsDiscriminator,
        ...multisigAccount,
      }).byteSize;

    // Newsignerwas added.
    assert.strictEqual(newMembersLength, initialMembersLength + 1);
    assert.ok(
      unwrapSigners(multisigAccount.signers).find((m) => m.__kind === "Native" && m.key.equals(newSigner.key))
    );
    // Account occupied size increased by the size of the new Member.
    // Use the actual beet-computed increase (packed format differs from standalone signerBeet)
    const perSignerOccupiedIncrease = newOccupiedSize - initialOccupiedSize;
    assert.ok(perSignerOccupiedIncrease > 30 && perSignerOccupiedIncrease < 50,
      `per-signer size should be 30-50 bytes, got ${perSignerOccupiedIncrease}`);
    // Account allocated size increased by the size of 1 Member
    assert.strictEqual(
      multisigAccountInfo!.data.length,
      initialAllocatedSize + perSignerOccupiedIncrease
    );

    // Adding one moresignershouldn't increase the allocated size.
    signature = await smartAccount.rpc.addSignerAsAuthority({
      connection,
      feePayer,
      settingsPda,
      settingsAuthority: configAuthority.publicKey,
      rentPayer: configAuthority,
      newSigner: newMember2,
      signers: [configAuthority],
      sendOptions: { skipPreflight: true },
      programId,
    });
    await connection.confirmTransaction(signature);
    // Re-fetch the smart account account.
    multisigAccountInfo = await connection.getAccountInfo(settingsPda);
    multisigAccount = Settings.fromAccountInfo(multisigAccountInfo!)[0];
    newMembersLength = unwrapSigners(multisigAccount.signers).length;
    newOccupiedSize = smartAccount.generated.settingsBeet.toFixedFromValue({
      accountDiscriminator: smartAccount.generated.settingsDiscriminator,
      ...multisigAccount,
    }).byteSize;
    // Added one more member.
    assert.strictEqual(newMembersLength, initialMembersLength + 2);
    assert.ok(
      unwrapSigners(multisigAccount.signers).find((m) => m.__kind === "Native" && m.key.equals(newMember2.key))
    );
    // Account occupied size increased by the size of one more Member.
    assert.strictEqual(
      newOccupiedSize,
      initialOccupiedSize + 2 * perSignerOccupiedIncrease
    );
    // Account allocated size increased by the size of 1 Member again.
    assert.strictEqual(
      multisigAccountInfo!.data.length,
      initialAllocatedSize + 2 * perSignerOccupiedIncrease
    );
  });
});
