import { Keypair, PublicKey } from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import assert from "assert";
import {
  createControlledMultisig,
  createLocalhostConnection,
  generateFundedKeypair,
  generateMultisigMembers,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
} from "../../utils";

const { Settings } = multisig.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / multisig_set_rent_collector", () => {
  let members: TestMembers;
  let settingsPda: PublicKey;
  let configAuthority: Keypair;

  before(async () => {
    configAuthority = await generateFundedKeypair(connection);
    const accountIndex = await getNextAccountIndex(connection, programId);
    members = await generateMultisigMembers(connection);

    // Create new controlled multisig with no rent_collector.
    settingsPda = (
      await createControlledMultisig({
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

  it("set `rent_collector` for the controlled multisig", async () => {
    const multisigAccountInfoPreExecution = await connection.getAccountInfo(
      settingsPda
    )!;

    const vaultPda = multisig.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    })[0];

    assert.rejects(
      async () =>
        await multisig.rpc.setRentCollectorAsAuthority({
          connection,
          settingsPda,
          feePayer: configAuthority,
          settingsAuthority: configAuthority.publicKey,
          newArchivalAuthority: vaultPda,
          programId,
          signers: [configAuthority],
        }), /NotImplemented/
    );

    // Verify the multisig account.
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
    // multisig space should not be reallocated because we allocate 32 bytes for potential rent_collector when we create multisig.
    assert.ok(
      multisigAccountInfoPostExecution!.data.length ===
      multisigAccountInfoPreExecution!.data.length
    );
  });

  it("unset `rent_collector` for the controlled multisig", async () => {
    assert.rejects(
      async () =>
        await multisig.rpc.setRentCollectorAsAuthority({
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
