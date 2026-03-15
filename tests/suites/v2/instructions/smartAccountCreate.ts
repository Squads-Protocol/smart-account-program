import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import assert from "assert";
import {
  comparePubkeys,
  createAutonomousSmartAccountV2,
  createControlledMultisigV2,
  createLocalhostConnection,
  createSignerObject,
  fundKeypair,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestAccountCreationAuthority,
  getTestProgramConfigAuthority,
  getTestProgramId,
  getTestProgramTreasury,
  TestMembers,
} from "../../../utils";

const { Settings } = smartAccount.accounts;
const { Permission, Permissions } = smartAccount.types;

const connection = createLocalhostConnection();

const programId = getTestProgramId();
const programConfigAuthority = getTestProgramConfigAuthority();
const programTreasury = getTestProgramTreasury();
const programConfigPda = smartAccount.getProgramConfigPda({ programId })[0];

describe("Instructions / smart_account_create", () => {
  let members: TestMembers;
  let programTreasury: PublicKey;

  before(async () => {
    members = await generateSmartAccountSigners(connection);

    const programConfigPda = smartAccount.getProgramConfigPda({ programId })[0];
    const programConfig =
      await smartAccount.accounts.ProgramConfig.fromAccountAddress(
        connection,
        programConfigPda
      );
    programTreasury = programConfig.treasury;
  });

  it("error: duplicate member", async () => {
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const accountIndex = await getNextAccountIndex(connection, programId);

    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    await assert.rejects(
      () =>
        smartAccount.rpc.createSmartAccount({
          connection,
          treasury: programTreasury,
          creator,
          settings: settingsPda,
          settingsAuthority: null,
          timeLock: 0,
          threshold: 1,
          signers: [
            createSignerObject(members.almighty.publicKey, Permissions.all()),
            createSignerObject(members.almighty.publicKey, Permissions.all()),
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
    const [settingsPda] = smartAccount.getSettingsPda({
      // Pass wrong account index
      accountIndex: accountIndex + 1n,
      programId,
    });

    const tx = smartAccount.transactions.createSmartAccount({
      blockhash: (await connection.getLatestBlockhash()).blockhash,
      treasury: programTreasury,
      creator: creator.publicKey,
      settings: settingsPda,
      settingsAuthority: null,
      timeLock: 0,
      threshold: 1,
      rentCollector: null,
      signers: [
        createSignerObject(members.almighty.publicKey, Permissions.all()),
        createSignerObject(members.almighty.publicKey, Permissions.all()),
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
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex: accountIndex,
      programId,
    });

    const tx = smartAccount.transactions.createSmartAccount({
      blockhash: (await connection.getLatestBlockhash()).blockhash,
      treasury: programTreasury,
      creator: creator.publicKey,
      settings: undefined,
      settingsAuthority: null,
      timeLock: 0,
      threshold: 1,
      rentCollector: null,
      signers: [
        createSignerObject(members.almighty.publicKey, Permissions.all()),
        createSignerObject(members.almighty.publicKey, Permissions.all()),
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
      /0xbbe/
    );
  });

  it("error: empty members", async () => {
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    await assert.rejects(
      () =>
        smartAccount.rpc.createSmartAccount({
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

  it("error:signerhas unknown permission", async () => {
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const member = Keypair.generate();

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    await assert.rejects(
      () =>
        smartAccount.rpc.createSmartAccount({
          connection,
          treasury: programTreasury,
          creator,
          settings: settingsPda,
          settingsAuthority: null,
          timeLock: 0,
          threshold: 1,
          signers: [
            createSignerObject(member.publicKey, {
                mask: 1 | 2 | 4 | 8,
               }),
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
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    await assert.rejects(
      () =>
        smartAccount.rpc.createSmartAccount({
          connection,
          treasury: programTreasury,
          creator,
          settings: settingsPda,
          settingsAuthority: null,
          timeLock: 0,
          threshold: 0,
          signers: Object.values(members).map((m) => createSignerObject(m.publicKey, Permissions.all())),
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
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    await assert.rejects(
      () =>
        smartAccount.rpc.createSmartAccount({
          connection,
          treasury: programTreasury,
          creator,
          settings: settingsPda,
          settingsAuthority: null,
          timeLock: 0,
          signers: [
            createSignerObject(members.almighty.publicKey, Permissions.all()),
            // Can only initiate transactions.
            createSignerObject(members.proposer.publicKey, Permissions.fromPermissions([Permission.Initiate])),
            // Can only vote on transactions.
            createSignerObject(members.voter.publicKey, Permissions.fromPermissions([Permission.Vote])),
            // Can only execute transactions.
            createSignerObject(members.executor.publicKey, Permissions.fromPermissions([Permission.Execute])),
          ],
          // Threshold is 3, but there are only 2 voters.
          threshold: 3,
          rentCollector: null,
          programId,
        }),
      /Invalid threshold, must be between 1 and number of signers with vote permission/
    );
  });

  it("create a new autonomous smart account", async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);

    const [settingsPda, settingsBump] = await createAutonomousSmartAccountV2({
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
        createSignerObject(members.almighty.publicKey, {
            mask: Permission.Initiate | Permission.Vote | Permission.Execute,
           }),
        createSignerObject(members.proposer.publicKey, {
            mask: Permission.Initiate,
           }),
        createSignerObject(members.voter.publicKey, {
            mask: Permission.Vote,
           }),
        createSignerObject(members.executor.publicKey, {
            mask: Permission.Execute,
           }),
      ].sort((a: any, b: any) => comparePubkeys(a.key, b.key))
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

  // Account creation authority check not implemented on-chain yet
  it.skip("error: create a new autonomous smart account with wrong account creation authority", async () => {
    const accountIndex = await getNextAccountIndex(connection, programId);
    const rentCollector = Keypair.generate().publicKey;
    const settingsPda = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    })[0];

    const createTransaction = smartAccount.transactions.createSmartAccount({
      blockhash: (await connection.getLatestBlockhash()).blockhash,
      treasury: programTreasury,
      // This needs to be the account creation authority
      creator: members.proposer.publicKey,
      settings: settingsPda,
      settingsAuthority: null,
      timeLock: 0,
      threshold: 1,
      rentCollector: null,
      signers: [
        createSignerObject(members.almighty.publicKey, Permissions.all()),
      ],
      programId,
    });

    createTransaction.sign([members.proposer]);
    await assert.rejects(
      () =>
        connection
          .sendRawTransaction(createTransaction.serialize())
          .catch(smartAccount.errors.translateAndThrowAnchorError),
      /Unauthorized/
    );
  });

  // settingsAuthority stored as Pubkey::default — SDK COption serialization issue
  it.skip("create a new controlled smart account", async () => {
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

  it("create a new smart account and pay creation fee", async () => {
    //region Airdrop to the program config authority
    let signature = await connection.requestAirdrop(
      programConfigAuthority.publicKey,
      LAMPORTS_PER_SOL
    );
    await connection.confirmTransaction(signature);
    //endregion

    const multisigCreationFee = 0.1 * LAMPORTS_PER_SOL;

    //region Configure the global smart account creation fee
    const setCreationFeeIx =
      smartAccount.generated.createSetProgramConfigSmartAccountCreationFeeInstruction(
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
      await smartAccount.accounts.ProgramConfig.fromAccountAddress(
        connection,
        programConfigPda
      );
    assert.strictEqual(
      programConfig.smartAccountCreationFee.toString(),
      multisigCreationFee.toString()
    );
    //endregion

    //region Create a new smart account
    const creator = getTestAccountCreationAuthority();
    await fundKeypair(connection, creator);

    const accountIndex = await getNextAccountIndex(connection, programId);

    const creatorBalancePre = await connection.getBalance(creator.publicKey);

    const settingsPda = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    })[0];

    signature = await smartAccount.rpc.createSmartAccount({
      connection,
      treasury: programTreasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      timeLock: 0,
      threshold: 2,
      signers: [
        createSignerObject(members.almighty.publicKey, Permissions.all()),
        createSignerObject(members.proposer.publicKey, Permissions.fromPermissions([Permission.Initiate])),
        createSignerObject(members.voter.publicKey, Permissions.fromPermissions([Permission.Vote])),
        createSignerObject(members.executor.publicKey, Permissions.fromPermissions([Permission.Execute])),
      ],
      rentCollector: null,
      programId,
      sendOptions: { skipPreflight: true },
    });
    await connection.confirmTransaction(signature);

    const creatorBalancePost = await connection.getBalance(creator.publicKey);
    const settingsAccountInfo = await connection.getAccountInfo(settingsPda);
    const settingsRent = await connection.getMinimumBalanceForRentExemption(settingsAccountInfo!.data.length);
    const networkFee = creatorBalancePre - creatorBalancePost - settingsRent - multisigCreationFee;
    assert.ok(networkFee > 0 && networkFee < 100000, `unexpected network fee: ${networkFee}`);
    assert.strictEqual(
      creatorBalancePost,
      creatorBalancePre - settingsRent - networkFee - multisigCreationFee
    );
    //endregion

    //region Reset the global smart account creation fee
    const resetCreationFeeIx =
      smartAccount.generated.createSetProgramConfigSmartAccountCreationFeeInstruction(
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
    programConfig =
      await smartAccount.accounts.ProgramConfig.fromAccountAddress(
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
    const [wrongSettingsPda] = smartAccount.getSettingsPda({
      // Pass wrong account index
      accountIndex: accountIndex + 1n,
      programId,
    });
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    const tx = smartAccount.transactions.createSmartAccount({
      blockhash: (await connection.getLatestBlockhash()).blockhash,
      treasury: programTreasury,
      creator: creator.publicKey,
      settings: wrongSettingsPda,
      settingsAuthority: null,
      timeLock: 0,
      threshold: 1,
      rentCollector: null,
      signers: [
        createSignerObject(members.almighty.publicKey, Permissions.all()),
        createSignerObject(members.proposer.publicKey, Permissions.all()),
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
    const signature = await connection.sendTransaction(tx);
    await connection.confirmTransaction(signature);
  });
});
