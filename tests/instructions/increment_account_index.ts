import * as smartAccount from "@sqds/smart-account";
import { PublicKey } from "@solana/web3.js";
import assert from "assert";
import {
  createLocalhostConnection,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getTestProgramId,
  TestMembers,
} from "../utils";
import {
  createSettings as createSettingsHelper,
  createSettingsV2 as createSettingsV2Helper,
  createSettingsV2WithEd25519External,
  createSettingsV2WithWebAuthn,
  createSettingsV2WithSecp256k1,
} from "./utils/settings";

const { Settings } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / increment_account_index", () => {
  const skip = { it: it.skip };
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  const createSettings = (timeLock = 0) =>
    createSettingsHelper({ connection, members, programId, timeLock });
  const createSettingsV2 = (timeLock = 0) =>
    createSettingsV2Helper({ connection, members, programId, timeLock });

  const getAccountUtilization = async (settingsPda: PublicKey) => {
    const settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    return settingsAccount.accountUtilization;
  };

  const incrementAccountIndexV1 = async (settingsPda: PublicKey) => {
    const signature = await smartAccount.rpc.incrementAccountIndex({
      connection,
      feePayer: members.proposer,
      settings: settingsPda,
      signer: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);
  };

  const incrementAccountIndexV2 = async (settingsPda: PublicKey) => {
    const signature = await smartAccount.rpc.incrementAccountIndexV2({
      connection,
      feePayer: members.proposer,
      settings: settingsPda,
      signerKey: members.proposer.publicKey,
      clientDataParams: null,
      anchorRemainingAccounts: [
        {
          pubkey: members.proposer.publicKey,
          isSigner: true,
          isWritable: false,
        },
      ],
      programId,
    });
    await connection.confirmTransaction(signature);
  };

  const createSettingsWithProposerPermissions = async (
    permissions: smartAccount.types.Permissions
  ) => {
    const programConfig =
      await smartAccount.accounts.ProgramConfig.fromAccountAddress(
        connection,
        smartAccount.getProgramConfigPda({ programId })[0]
      );
    const accountIndex = BigInt(programConfig.smartAccountIndex.toString()) + 1n;
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });
    const signature = await smartAccount.rpc.createSmartAccount({
      connection,
      treasury: programConfig.treasury,
      creator: members.almighty,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [
        {
          key: members.almighty.publicKey,
          permissions: smartAccount.types.Permissions.all(),
        },
        {
          key: members.proposer.publicKey,
          permissions,
        },
      ],
      timeLock: 0,
      rentCollector: null,
      sendOptions: { skipPreflight: true },
      programId,
    });
    await connection.confirmTransaction(signature);

    return settingsPda;
  };

  // -------------------------------------------------------------------------------------
  // Golden Path Tests (V1 + V2 parity)
  // -------------------------------------------------------------------------------------

  it("should_increment_account_index_v1", async () => {
    const settingsPda = await createSettings();
    const before = await getAccountUtilization(settingsPda);

    await incrementAccountIndexV1(settingsPda);

    const after = await getAccountUtilization(settingsPda);
    assert.strictEqual(after, before + 1);
  });

  it("should_increment_account_index_v2", async () => {
    const settingsPda = await createSettings();
    const before = await getAccountUtilization(settingsPda);

    await incrementAccountIndexV2(settingsPda);

    const after = await getAccountUtilization(settingsPda);
    assert.strictEqual(after, before + 1);
  });

  it("should_increment_account_utilization_field_v1", async () => {
    const settingsPda = await createSettings();
    const before = await getAccountUtilization(settingsPda);

    await incrementAccountIndexV1(settingsPda);
    await incrementAccountIndexV1(settingsPda);

    const after = await getAccountUtilization(settingsPda);
    assert.strictEqual(after, before + 2);
  });

  it("should_increment_account_utilization_field_v2", async () => {
    const settingsPda = await createSettings();
    const before = await getAccountUtilization(settingsPda);

    await incrementAccountIndexV2(settingsPda);
    await incrementAccountIndexV2(settingsPda);

    const after = await getAccountUtilization(settingsPda);
    assert.strictEqual(after, before + 2);
  });

  // -------------------------------------------------------------------------------------
  // Invariant & Safety Tests (V1 + V2 parity)
  // -------------------------------------------------------------------------------------

  it("should_fail_on_not_a_signer_v1", async () => {
    const settingsPda = await createSettings();
    const outsider = await generateFundedKeypair(connection);

    await assert.rejects(
      () =>
        smartAccount.rpc.incrementAccountIndex({
          connection,
          feePayer: outsider,
          settings: settingsPda,
          signer: outsider,
          programId,
        }),
      /NotASigner/
    );
  });

  it("should_fail_on_not_a_signer_v2", async () => {
    const settingsPda = await createSettings();
    const outsider = await generateFundedKeypair(connection);

    await assert.rejects(
      () =>
        smartAccount.rpc.incrementAccountIndexV2({
          connection,
          feePayer: outsider,
          settings: settingsPda,
          signerKey: outsider.publicKey,
          clientDataParams: null,
          anchorRemainingAccounts: [
            {
              pubkey: outsider.publicKey,
              isSigner: true,
              isWritable: false,
            },
          ],
          signers: [outsider],
          programId,
        }),
      /NotASigner/
    );
  });
  
  it("should_fail_on_missing_any_permission_v1", async () => {
    const settingsPda = await createSettingsWithProposerPermissions(
      smartAccount.types.Permissions.fromPermissions([])
    );

    await assert.rejects(
      () =>
        smartAccount.rpc.incrementAccountIndex({
          connection,
          feePayer: members.proposer,
          settings: settingsPda,
          signer: members.proposer,
          programId,
        }),
      /Unauthorized/
    );
  });

  it("should_fail_on_missing_any_permission_v2", async () => {
    const settingsPda = await createSettingsWithProposerPermissions(
      smartAccount.types.Permissions.fromPermissions([])
    );

    await assert.rejects(
      () =>
        smartAccount.rpc.incrementAccountIndexV2({
          connection,
          feePayer: members.proposer,
          settings: settingsPda,
          signerKey: members.proposer.publicKey,
          clientDataParams: null,
          anchorRemainingAccounts: [
            {
              pubkey: members.proposer.publicKey,
              isSigner: true,
              isWritable: false,
            },
          ],
          signers: [members.proposer],
          programId,
        }),
      /Unauthorized/
    );
  });

  skip.it("should_fail_on_max_account_index_reached_v1");
  skip.it("should_fail_on_max_account_index_reached_v2");

  it("should_fail_on_invalid_v2_context_signature_v2", async () => {
    const settingsPda = await createSettings();

    await assert.rejects(
      () =>
        smartAccount.rpc.incrementAccountIndexV2({
          connection,
          feePayer: members.proposer,
          settings: settingsPda,
          signerKey: members.proposer.publicKey,
          clientDataParams: null,
          anchorRemainingAccounts: [],
          signers: [members.proposer],
          programId,
        }),
      /Missing signature|MissingSignature/
    );
  });
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");

  // -------------------------------------------------------------------------------------
  // Edge Case Tests (V1 + V2 parity)
  // -------------------------------------------------------------------------------------

  it("should_increment_from_zero_v1", async () => {
    const settingsPda = await createSettings();
    const before = await getAccountUtilization(settingsPda);
    assert.strictEqual(before, 0);

    await incrementAccountIndexV1(settingsPda);

    const after = await getAccountUtilization(settingsPda);
    assert.strictEqual(after, 1);
  });

  it("should_increment_from_zero_v2", async () => {
    const settingsPda = await createSettings();
    const before = await getAccountUtilization(settingsPda);
    assert.strictEqual(before, 0);

    await incrementAccountIndexV2(settingsPda);

    const after = await getAccountUtilization(settingsPda);
    assert.strictEqual(after, 1);
  });
  skip.it("should_increment_at_max_minus_one_v1");
  skip.it("should_increment_at_max_minus_one_v2");

  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  // TODO: Enable external signer V2 settings tests once external signer serialization is fixed.

  skip.it("should_increment_account_index_v2_with_native_settings_v2", async () => {
    const settingsPda = await createSettingsV2();
    const before = await getAccountUtilization(settingsPda);

    await incrementAccountIndexV2(settingsPda);

    const after = await getAccountUtilization(settingsPda);
    assert.strictEqual(after, before + 1);
  });

  skip.it("should_increment_account_index_v2_with_p256_webauthn_settings_v2", async () => {
    const { settingsPda } = await createSettingsV2WithWebAuthn({
      connection,
      members,
      programId,
    });
    const before = await getAccountUtilization(settingsPda);

    await incrementAccountIndexV2(settingsPda);

    const after = await getAccountUtilization(settingsPda);
    assert.strictEqual(after, before + 1);
  });

  skip.it("should_increment_account_index_v2_with_secp256k1_settings_v2", async () => {
    const { settingsPda } = await createSettingsV2WithSecp256k1({
      connection,
      members,
      programId,
    });
    const before = await getAccountUtilization(settingsPda);

    await incrementAccountIndexV2(settingsPda);

    const after = await getAccountUtilization(settingsPda);
    assert.strictEqual(after, before + 1);
  });

  skip.it("should_increment_account_index_v2_with_ed25519_external_settings_v2", async () => {
    const { settingsPda } = await createSettingsV2WithEd25519External({
      connection,
      members,
      programId,
    });
    const before = await getAccountUtilization(settingsPda);

    await incrementAccountIndexV2(settingsPda);

    const after = await getAccountUtilization(settingsPda);
    assert.strictEqual(after, before + 1);
  });

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
