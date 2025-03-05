import { Keypair, PublicKey } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import assert from "assert";
import {
  createControlledSmartAccount,
  createLocalhostConnection,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const { Settings } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / smart_account_set_archival_authority", () => {
  let members: TestMembers;
  let settingsPda: PublicKey;
  let configAuthority: Keypair;

  before(async () => {
    configAuthority = await generateFundedKeypair(connection);
    const accountIndex = await getNextAccountIndex(connection, programId);
    members = await generateSmartAccountSigners(connection);

    // Create new controlled smart account with no rent_collector.
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

  it("set `archival_authority` for the controlled smart account", async () => {
    const multisigAccountInfoPreExecution = await connection.getAccountInfo(
      settingsPda
    )!;

    const vaultPda = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    })[0];

    assert.rejects(
      async () =>
        await smartAccount.rpc.setArchivalAuthorityAsAuthority({
          connection,
          settingsPda,
          feePayer: configAuthority,
          settingsAuthority: configAuthority.publicKey,
          newArchivalAuthority: vaultPda,
          programId,
          signers: [configAuthority],
        }),
      /NotImplemented/
    );

    // Verify the smart account account.
    const multisigAccountInfoPostExecution = await connection.getAccountInfo(
      settingsPda
    );
    const [multisigAccountPostExecution] = Settings.fromAccountInfo(
      multisigAccountInfoPostExecution!
    );
    // The stale transaction index should NOT be updated and remain 0.
    assert.strictEqual(
      multisigAccountPostExecution.staleTransactionIndex.toString(),
      "0"
    );
    // smart account space should not be reallocated because we allocate 32 bytes for potential rent_collector when we create smartAccount.
    assert.ok(
      multisigAccountInfoPostExecution!.data.length ===
        multisigAccountInfoPreExecution!.data.length
    );
  });

  it("unset `archival_authority` for the controlled smart account", async () => {
    assert.rejects(
      async () =>
        await smartAccount.rpc.setArchivalAuthorityAsAuthority({
          connection,
          settingsPda,
          feePayer: configAuthority,
          settingsAuthority: configAuthority.publicKey,
          newArchivalAuthority: null,
          programId,
          signers: [configAuthority],
        }),
      /NotImplemented/
    );

    // // Make sure the rent_collector was unset correctly.
    // const multisigAccount = await Settings.fromAccountAddress(
    //   connection,
    //   settingsPda
    // );
    // assert.strictEqual(multisigAccount.rentCollector, null);
  });
});
