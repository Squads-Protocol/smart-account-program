import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import assert from "assert";
import {
  comparePubkeys,
  createAutonomousMultisigV2,
  createControlledMultisigV2,
  createLocalhostConnection,
  fundKeypair,
  generateFundedKeypair,
  generateMultisigMembers,
  getNextAccountIndex,
  getTestAccountCreationAuthority,
  getTestProgramConfigAuthority,
  getTestProgramId,
  getTestProgramTreasury,
  TestMembers,
} from "../../utils";

const { Settings } = multisig.accounts;
const { Permission, Permissions } = multisig.types;

const connection = createLocalhostConnection();

const programId = getTestProgramId();
const programConfigAuthority = getTestProgramConfigAuthority();
const programTreasury = getTestProgramTreasury();
const programConfigPda = multisig.getProgramConfigPda({ programId })[0];

describe("Instructions / multisig_create_v2", () => {
  let members: TestMembers;
  let programTreasury: PublicKey;

  before(async () => {
    members = await generateMultisigMembers(connection);

    const programConfigPda = multisig.getProgramConfigPda({ programId })[0];
    const programConfig =
      await multisig.accounts.ProgramConfig.fromAccountAddress(
        connection,
        programConfigPda
      );
    programTreasury = programConfig.treasury;
  });

  it("error: duplicate member", async () => {
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const accountIndex = await getNextAccountIndex(connection, programId);

    const [settingsPda] = multisig.getSettingsPda({
      accountIndex,
      programId,
    });

    await assert.rejects(
      () =>
        multisig.rpc.createSmartAccount({
          connection,
          treasury: programTreasury,
          creator,
          settings: settingsPda,
          settingsAuthority: null,
          timeLock: 0,
          threshold: 1,
          signers: [
            {
              key: members.almighty.publicKey,
              permissions: Permissions.all(),
            },
            {
              key: members.almighty.publicKey,
              permissions: Permissions.all(),
            },
          ],
          rentCollector: null,
          programId,
        }),
      /Found multiple signers with the same pubkey/
    );
  });

  it("error: invalid settings account address", async () => {
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = multisig.getSettingsPda({
      // Pass wrong account index
      accountIndex: accountIndex + 1n,
      programId,
    });

    const tx = multisig.transactions.createSmartAccount({
      blockhash: (await connection.getLatestBlockhash()).blockhash,
      treasury: programTreasury,
      creator: creator.publicKey,
      settings: settingsPda,
      settingsAuthority: null,
      timeLock: 0,
      threshold: 1,
      rentCollector: null,
      signers: [
        {
          key: members.almighty.publicKey,
          permissions: Permissions.all(),
        },
        {
          key: members.almighty.publicKey,
          permissions: Permissions.all(),
        },
      ],
      programId,
    });

    tx.sign([creator]);

    // 0x7d6 is ConstraintSeeds
    await assert.rejects(
      async () => await connection.sendTransaction(tx),
      /0x1788/
    );
  });
  it("error: settings address not passed as writable", async () => {
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = multisig.getSettingsPda({
      accountIndex: accountIndex,
      programId,
    });

    const tx = multisig.transactions.createSmartAccount({
      blockhash: (await connection.getLatestBlockhash()).blockhash,
      treasury: programTreasury,
      creator: creator.publicKey,
      settings: undefined,
      settingsAuthority: null,
      timeLock: 0,
      threshold: 1,
      rentCollector: null,
      signers: [
        {
          key: members.almighty.publicKey,
          permissions: Permissions.all(),
        },
        {
          key: members.almighty.publicKey,
          permissions: Permissions.all(),
        },
      ],
      programId,
      remainingAccounts: [
        {
          pubkey: settingsPda,
          isSigner: false,
          // Passed as non-writable
          isWritable: false,
        },
      ],
    });

    tx.sign([creator]);

    // 3006 is AccountNotMutable
    await assert.rejects(
      async () => await connection.sendTransaction(tx),
      /3006/
    );
  });

  it("error: empty members", async () => {
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = multisig.getSettingsPda({
      accountIndex,
      programId,
    });

    await assert.rejects(
      () =>
        multisig.rpc.createSmartAccount({
          connection,
          treasury: programTreasury,
          creator,
          settings: settingsPda,
          settingsAuthority: null,
          timeLock: 0,
          threshold: 1,
          signers: [],
          rentCollector: null,
          programId,
        }),
      /Signers don't include any proposers/
    );
  });

  it("error: member has unknown permission", async () => {
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const member = Keypair.generate();

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = multisig.getSettingsPda({
      accountIndex,
      programId,
    });

    await assert.rejects(
      () =>
        multisig.rpc.createSmartAccount({
          connection,
          treasury: programTreasury,
          creator,
          settings: settingsPda,
          settingsAuthority: null,
          timeLock: 0,
          threshold: 1,
          signers: [
            {
              key: member.publicKey,
              permissions: {
                mask: 1 | 2 | 4 | 8,
              },
            },
          ],
          rentCollector: null,
          programId,
        }),
      /Signer has unknown permission/
    );
  });

  // We cannot really test it because we can't pass u16::MAX members to the instruction.
  it("error: too many members");

  it("error: invalid threshold (< 1)", async () => {
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = multisig.getSettingsPda({
      accountIndex,
      programId,
    });

    await assert.rejects(
      () =>
        multisig.rpc.createSmartAccount({
          connection,
          treasury: programTreasury,
          creator,
          settings: settingsPda,
          settingsAuthority: null,
          timeLock: 0,
          threshold: 0,
          signers: Object.values(members).map((m) => ({
            key: m.publicKey,
            permissions: Permissions.all(),
          })),
          rentCollector: null,
          programId,
        }),
      /Invalid threshold, must be between 1 and number of signers/
    );
  });

  it("error: invalid threshold (> members with permission to Vote)", async () => {
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = multisig.getSettingsPda({
      accountIndex,
      programId,
    });

    await assert.rejects(
      () =>
        multisig.rpc.createSmartAccount({
          connection,
          treasury: programTreasury,
          creator,
          settings: settingsPda,
          settingsAuthority: null,
          timeLock: 0,
          signers: [
            {
              key: members.almighty.publicKey,
              permissions: Permissions.all(),
            },
            // Can only initiate transactions.
            {
              key: members.proposer.publicKey,
              permissions: Permissions.fromPermissions([Permission.Initiate]),
            },
            // Can only vote on transactions.
            {
              key: members.voter.publicKey,
              permissions: Permissions.fromPermissions([Permission.Vote]),
            },
            // Can only execute transactions.
            {
              key: members.executor.publicKey,
              permissions: Permissions.fromPermissions([Permission.Execute]),
            },
          ],
          // Threshold is 3, but there are only 2 voters.
          threshold: 3,
          rentCollector: null,
          programId,
        }),
      /Invalid threshold, must be between 1 and number of signers with vote permission/
    );
  });

  it("create a new autonomous multisig", async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);

    const [settingsPda, settingsBump] = await createAutonomousMultisigV2({
      connection,
      accountIndex,
      members,
      threshold: 2,
      timeLock: 0,
      rentCollector: null,
      programId,
    });

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(
      multisigAccount.settingsAuthority.toBase58(),
      PublicKey.default.toBase58()
    );
    assert.strictEqual(multisigAccount.threshold, 2);
    assert.deepEqual(
      multisigAccount.signers,
      [
        {
          key: members.almighty.publicKey,
          permissions: {
            mask: Permission.Initiate | Permission.Vote | Permission.Execute,
          },
        },
        {
          key: members.proposer.publicKey,
          permissions: {
            mask: Permission.Initiate,
          },
        },
        {
          key: members.voter.publicKey,
          permissions: {
            mask: Permission.Vote,
          },
        },
        {
          key: members.executor.publicKey,
          permissions: {
            mask: Permission.Execute,
          },
        },
      ].sort((a, b) => comparePubkeys(a.key, b.key))
    );
    assert.strictEqual(
      multisigAccount.archivalAuthority?.toBase58(),
      PublicKey.default.toBase58()
    );
    assert.strictEqual(multisigAccount.archivableAfter.toString(), "0");
    assert.strictEqual(multisigAccount.transactionIndex.toString(), "0");
    assert.strictEqual(multisigAccount.staleTransactionIndex.toString(), "0");
    assert.strictEqual(
      multisigAccount.seed.toString(),
      accountIndex.toString()
    );
    assert.strictEqual(multisigAccount.bump, settingsBump);
  });

  it("error: create a new autonomous multisig with wrong account creation authority", async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);
    const rentCollector = Keypair.generate().publicKey;
    const settingsPda = multisig.getSettingsPda({
      accountIndex,
      programId,
    })[0];

    const createTransaction = multisig.transactions.createSmartAccount({
      blockhash: (await connection.getLatestBlockhash()).blockhash,
      treasury: programTreasury,
      // This needs to be the account creation authority
      creator: members.proposer.publicKey,
      settings: settingsPda,
      settingsAuthority: null,
      timeLock: 0,
      threshold: 2,
      rentCollector: null,
      signers: [
        { key: members.almighty.publicKey, permissions: Permissions.all() },
      ],
      programId,
    });

    assert.rejects(
      () => connection.sendTransaction(createTransaction),
      /Unauthorized/
    );
  });

  it("create a new controlled multisig", async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);
    const configAuthority = await generateFundedKeypair(connection);

    const [settingsPda] = await createControlledMultisigV2({
      connection,
      accountIndex,
      configAuthority: configAuthority.publicKey,
      members,
      threshold: 2,
      timeLock: 0,
      rentCollector: null,
      programId,
    });

    const multisigAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );

    assert.strictEqual(
      multisigAccount.settingsAuthority.toBase58(),
      configAuthority.publicKey.toBase58()
    );
    // We can skip the rest of the assertions because they are already tested
    // in the previous case and will be the same here.
  });

  it("create a new multisig and pay creation fee", async () => {
    //region Airdrop to the program config authority
    let signature = await connection.requestAirdrop(
      programConfigAuthority.publicKey,
      LAMPORTS_PER_SOL
    );
    await connection.confirmTransaction(signature);
    //endregion

    const multisigCreationFee = 0.1 * LAMPORTS_PER_SOL;

    //region Configure the global multisig creation fee
    const setCreationFeeIx =
      multisig.generated.createSetProgramConfigSmartAccountCreationFeeInstruction(
        {
          programConfig: programConfigPda,
          authority: programConfigAuthority.publicKey,
        },
        {
          args: { newSmartAccountCreationFee: multisigCreationFee },
        },
        programId
      );
    const message = new TransactionMessage({
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      payerKey: programConfigAuthority.publicKey,
      instructions: [setCreationFeeIx],
    }).compileToV0Message();
    const tx = new VersionedTransaction(message);
    tx.sign([programConfigAuthority]);
    signature = await connection.sendTransaction(tx);
    await connection.confirmTransaction(signature);
    let programConfig =
      await multisig.accounts.ProgramConfig.fromAccountAddress(
        connection,
        programConfigPda
      );
    assert.strictEqual(
      programConfig.smartAccountCreationFee.toString(),
      multisigCreationFee.toString()
    );
    //endregion

    //region Create a new multisig
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const accountIndex = await getNextAccountIndex(connection, programId);

    const creatorBalancePre = await connection.getBalance(creator.publicKey);

    const settingsPda = multisig.getSettingsPda({
      accountIndex,
      programId,
    })[0];

    signature = await multisig.rpc.createSmartAccount({
      connection,
      treasury: programTreasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      timeLock: 0,
      threshold: 2,
      signers: [
        { key: members.almighty.publicKey, permissions: Permissions.all() },
        {
          key: members.proposer.publicKey,
          permissions: Permissions.fromPermissions([Permission.Initiate]),
        },
        {
          key: members.voter.publicKey,
          permissions: Permissions.fromPermissions([Permission.Vote]),
        },
        {
          key: members.executor.publicKey,
          permissions: Permissions.fromPermissions([Permission.Execute]),
        },
      ],
      rentCollector: null,
      programId,
      sendOptions: { skipPreflight: true },
    });
    await connection.confirmTransaction(signature);

    const creatorBalancePost = await connection.getBalance(creator.publicKey);
    const rentAndNetworkFee = 2677640;

    assert.strictEqual(
      creatorBalancePost,
      creatorBalancePre - rentAndNetworkFee - multisigCreationFee
    );
    //endregion

    //region Reset the global multisig creation fee
    const resetCreationFeeIx =
      multisig.generated.createSetProgramConfigSmartAccountCreationFeeInstruction(
        {
          programConfig: programConfigPda,
          authority: programConfigAuthority.publicKey,
        },
        {
          args: { newSmartAccountCreationFee: 0 },
        },
        programId
      );
    const message2 = new TransactionMessage({
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      payerKey: programConfigAuthority.publicKey,
      instructions: [resetCreationFeeIx],
    }).compileToV0Message();
    const tx2 = new VersionedTransaction(message2);
    tx2.sign([programConfigAuthority]);
    signature = await connection.sendTransaction(tx2);
    await connection.confirmTransaction(signature);
    programConfig = await multisig.accounts.ProgramConfig.fromAccountAddress(
      connection,
      programConfigPda
    );
    assert.strictEqual(programConfig.smartAccountCreationFee.toString(), "0");
    //endregion
  });

  it("passing both an incorrect and correct settings account", async () => {
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [wrongSettingsPda] = multisig.getSettingsPda({
      // Pass wrong account index
      accountIndex: accountIndex + 1n,
      programId,
    });
    const [settingsPda] = multisig.getSettingsPda({
      accountIndex,
      programId,
    });

    const tx = multisig.transactions.createSmartAccount({
      blockhash: (await connection.getLatestBlockhash()).blockhash,
      treasury: programTreasury,
      creator: creator.publicKey,
      settings: wrongSettingsPda,
      settingsAuthority: null,
      timeLock: 0,
      threshold: 1,
      rentCollector: null,
      signers: [
        {
          key: members.almighty.publicKey,
          permissions: Permissions.all(),
        },
        {
          key: members.almighty.publicKey,
          permissions: Permissions.all(),
        },
      ],
      programId,
      remainingAccounts: [
        {
          pubkey: settingsPda,
          isSigner: false,
          isWritable: true,
        },
      ],
    });

    tx.sign([creator]);

    // Should still pass since the program looks through the remaining accounts
    await assert.ok(
      async () => await connection.sendTransaction(tx)
    );
  });
});
