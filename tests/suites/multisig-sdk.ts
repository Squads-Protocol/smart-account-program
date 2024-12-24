import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  TransactionMessage,
} from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import * as assert from "assert";
import {
  comparePubkeys,
  createAutonomousMultisig,
  createControlledMultisig,
  createLocalhostConnection,
  createTestTransferInstruction,
  generateFundedKeypair,
  generateMultisigMembers,
  getTestProgramId,
  isCloseToNow,
  TestMembers,
} from "../utils";
import BN from "bn.js";

const { toBigInt } = multisig.utils;
const { Settings, Transaction, SettingsTransaction, Proposal, SpendingLimit } =
  multisig.accounts;
const { Permission, Permissions } = multisig.types;

const programId = getTestProgramId();

describe("Multisig SDK", () => {
  const connection = createLocalhostConnection();

  let members: TestMembers;

  before(async () => {
    members = await generateMultisigMembers(connection);
  });

  describe("multisig_add_member", () => {
    const newSigner = {
      key: Keypair.generate().publicKey,
      permissions: Permissions.all(),
    } as const;
    const newMember2 = {
      key: Keypair.generate().publicKey,
      permissions: Permissions.all(),
    } as const;

    let settingsPda: PublicKey;
    let configAuthority: Keypair;

    before(async () => {
      configAuthority = await generateFundedKeypair(connection);

      // Create new controlled multisig.
      settingsPda = (
        await createControlledMultisig({
          connection,
          createKey: Keypair.generate(),
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

      // Adding the same member again should fail.
      await assert.rejects(
        multisig.rpc.addSignerAsAuthority({
          connection,
          feePayer,
          settingsPda,
          settingsAuthority: configAuthority.publicKey,
          rentPayer: configAuthority,
          newSigner: {
            key: members.almighty.publicKey,
            permissions: Permissions.all(),
          },
          signers: [configAuthority],
          programId,
        }),
        /Found multiple signers with the same pubkey/
      );
    });

    it("error: missing authority signature", async () => {
      const feePayer = await generateFundedKeypair(connection);

      await assert.rejects(
        multisig.rpc.addSignerAsAuthority({
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
        multisig.rpc.addSignerAsAuthority({
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

    it("add a new member to the controlled multisig", async () => {
      // feePayer can be anyone.
      const feePayer = await generateFundedKeypair(connection);

      let multisigAccountInfo = await connection.getAccountInfo(settingsPda);
      assert.ok(multisigAccountInfo);
      let [multisigAccount] = Settings.fromAccountInfo(multisigAccountInfo);

      const initialMembersLength = multisigAccount.signers.length;
      const initialOccupiedSize =
        multisig.generated.settingsBeet.toFixedFromValue({
          accountDiscriminator: multisig.generated.settingsDiscriminator,
          ...multisigAccount,
        }).byteSize;
      const initialAllocatedSize = multisigAccountInfo.data.length;

      // Right after the creation of the multisig, the allocated account space is almost fully utilized,
      // with only 32 bytes left for the potential rent collector.
      assert.equal(initialOccupiedSize, initialAllocatedSize - 32);

      let signature = await multisig.rpc.addSignerAsAuthority({
        connection,
        feePayer,
        settingsPda,
        settingsAuthority: configAuthority.publicKey,
        rentPayer: configAuthority,
        newSigner,
        memo: "Adding my good friend to the multisig",
        signers: [configAuthority],
        sendOptions: { skipPreflight: true },
        programId,
      });
      await connection.confirmTransaction(signature);

      multisigAccountInfo = await connection.getAccountInfo(settingsPda);
      multisigAccount = Settings.fromAccountInfo(multisigAccountInfo!)[0];

      let newMembersLength = multisigAccount.signers.length;
      let newOccupiedSize = multisig.generated.settingsBeet.toFixedFromValue({
        accountDiscriminator: multisig.generated.settingsDiscriminator,
        ...multisigAccount,
      }).byteSize;

      // New member was added.
      assert.strictEqual(newMembersLength, initialMembersLength + 1);
      assert.ok(
        multisigAccount.signers.find((m) => m.key.equals(newSigner.key))
      );
      // Account occupied size increased by the size of the new Member.
      assert.strictEqual(
        newOccupiedSize,
        initialOccupiedSize + multisig.generated.smartAccountSignerBeet.byteSize
      );
      // Account allocated size increased by the size of 1 Member
      assert.strictEqual(
        multisigAccountInfo!.data.length,
        initialAllocatedSize + 1 * multisig.generated.smartAccountSignerBeet.byteSize
      );

      // Adding one more member shouldn't increase the allocated size.
      signature = await multisig.rpc.addSignerAsAuthority({
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
      // Re-fetch the multisig account.
      multisigAccountInfo = await connection.getAccountInfo(settingsPda);
      multisigAccount = Settings.fromAccountInfo(multisigAccountInfo!)[0];
      newMembersLength = multisigAccount.signers.length;
      newOccupiedSize = multisig.generated.settingsBeet.toFixedFromValue({
        accountDiscriminator: multisig.generated.settingsDiscriminator,
        ...multisigAccount,
      }).byteSize;
      // Added one more member.
      assert.strictEqual(newMembersLength, initialMembersLength + 2);
      assert.ok(
        multisigAccount.signers.find((m) => m.key.equals(newMember2.key))
      );
      // Account occupied size increased by the size of one more Member.
      assert.strictEqual(
        newOccupiedSize,
        initialOccupiedSize + 2 * multisig.generated.smartAccountSignerBeet.byteSize
      );
      // Account allocated size increased by the size of 1 Member again.
      assert.strictEqual(
        multisigAccountInfo!.data.length,
        initialAllocatedSize + 2 * multisig.generated.smartAccountSignerBeet.byteSize
      );
    });
  });

  describe("multisig_batch_transactions", () => {
    const newSigner = {
      key: Keypair.generate().publicKey,
      permissions: Permissions.all(),
    } as const;
    const newMember2 = {
      key: Keypair.generate().publicKey,
      permissions: Permissions.all(),
    } as const;

    let settingsPda: PublicKey;
    let configAuthority: Keypair;

    before(async () => {
      configAuthority = await generateFundedKeypair(connection);

      // Create new controlled multisig.
      settingsPda = (
        await createControlledMultisig({
          connection,
          createKey: Keypair.generate(),
          configAuthority: configAuthority.publicKey,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];
    });

    it("create a batch transaction", async () => {
      const feePayer = await generateFundedKeypair(connection);

      const createBatchSignature = await multisig.rpc.createBatch({
        connection,
        batchIndex: 1n,
        creator: members.proposer,
        feePayer,
        settingsPda,
        accountIndex: 1,
        programId,
      });
      await connection.confirmTransaction(createBatchSignature);
    });
  });

  describe("multisig_config_transaction_set_time_lock", () => {
    let settingsPda: PublicKey;
    let configAuthority: Keypair;
    before(async () => {
      configAuthority = await generateFundedKeypair(connection);

      // Create new controlled multisig.
      settingsPda = (
        await createAutonomousMultisig({
          connection,
          createKey: Keypair.generate(),
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
        multisig.rpc.createSettingsTransaction({
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

    it("set `time_lock` for the autonomous multisig", async () => {
      const signature = await multisig.rpc.createSettingsTransaction({
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

  describe("multisig_set_time_lock", () => {
    let settingsPda: PublicKey;
    let configAuthority: Keypair;
    let wrongConfigAuthority: Keypair;
    before(async () => {
      configAuthority = await generateFundedKeypair(connection);
      wrongConfigAuthority = await generateFundedKeypair(connection);

      // Create new controlled multisig.
      settingsPda = (
        await createControlledMultisig({
          connection,
          createKey: Keypair.generate(),
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
        multisig.rpc.setTimeLockAsAuthority({
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

    it("set `time_lock` for the controlled multisig", async () => {
      const feePayer = await generateFundedKeypair(connection);
      const signature = await multisig.rpc.setTimeLockAsAuthority({
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

  describe("multisig_set_config_authority", () => {
    let settingsPda: PublicKey;
    let configAuthority: Keypair;

    before(async () => {
      configAuthority = await generateFundedKeypair(connection);

      // Create new controlled multisig.
      settingsPda = (
        await createControlledMultisig({
          connection,
          createKey: Keypair.generate(),
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
        multisig.rpc.setNewSettingsAuthorityAsAuthority({
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

    it("set `config authority for the controlled multisig", async () => {
      const feePayer = await generateFundedKeypair(connection);
      const signature = await multisig.rpc.setNewSettingsAuthorityAsAuthority({
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

  describe("multisig_remove_member", () => {
    let settingsPda: PublicKey;
    let configAuthority: Keypair;
    let wrongConfigAuthority: Keypair;
    before(async () => {
      configAuthority = await generateFundedKeypair(connection);
      wrongConfigAuthority = await generateFundedKeypair(connection);

      // Create new controlled multisig.
      settingsPda = (
        await createControlledMultisig({
          connection,
          createKey: Keypair.generate(),
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
        multisig.rpc.removeSignerAsAuthority({
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

    it("remove the member for the controlled multisig", async () => {
      const signature = await multisig.rpc.removeSignerAsAuthority({
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

  describe("multisig_change_threshold", () => {
    let settingsPda: PublicKey;
    let configAuthority: Keypair;

    before(async () => {
      configAuthority = await generateFundedKeypair(connection);

      // Create new controlled multisig.
      settingsPda = (
        await createAutonomousMultisig({
          connection,
          createKey: Keypair.generate(),
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
        multisig.rpc.createSettingsTransaction({
          connection,
          feePayer,
          settingsPda: settingsPda,
          transactionIndex: 1n,
          creator: members.proposer.publicKey,
          actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
          programId,
        })
      ),
        /Attempted to perform an unauthorized action/;
    });

    it("error: change threshold to higher amount than members", async () => {
      const feePayer = await generateFundedKeypair(connection);
      const configTransactionCreateSignature =
        await multisig.rpc.createSettingsTransaction({
          connection,
          feePayer,
          settingsPda: settingsPda,
          transactionIndex: 1n,
          creator: members.proposer.publicKey,
          actions: [{ __kind: "ChangeThreshold", newThreshold: 10 }],
          signers: [members.proposer, feePayer],
          programId,
        });
      await connection.confirmTransaction(configTransactionCreateSignature);

      const createProposalSignature = await multisig.rpc.createProposal({
        connection,
        creator: members.proposer,
        settingsPda,
        feePayer,
        transactionIndex: 1n,
        isDraft: false,
        programId,
      });
      await connection.confirmTransaction(createProposalSignature);

      const approveSignature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex: 1n,
        signer: members.voter,
        programId,
      });
      await connection.confirmTransaction(approveSignature);

      await assert.rejects(
        multisig.rpc.executeSettingsTransaction({
          connection,
          feePayer,
          settingsPda: settingsPda,
          transactionIndex: 1n,
          signer: members.executor,
          rentPayer: feePayer,
          programId,
        }),
        /Invalid threshold, must be between 1 and number of signers with vote permission/
      );
    });

    it("change `threshold` for the controlled multisig", async () => {
      const signature = await multisig.rpc.createSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda: settingsPda,
        transactionIndex: 2n,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
        programId,
      });
      await connection.confirmTransaction(signature);
    });
  });

  describe("multisig_config_transaction_remove_member", () => {
    let settingsPda: PublicKey;
    let configAuthority: Keypair;
    before(async () => {
      configAuthority = await generateFundedKeypair(connection);

      // Create new controlled multisig.
      settingsPda = (
        await createAutonomousMultisig({
          connection,
          createKey: Keypair.generate(),
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];
    });
    it("error: invalid authority", async () => {
      const feePayer = await generateFundedKeypair(connection);
      await assert.rejects(
        multisig.rpc.createSettingsTransaction({
          connection,
          feePayer,
          settingsPda: settingsPda,
          transactionIndex: 1n,
          creator: members.proposer.publicKey,
          actions: [
            { __kind: "RemoveSigner", oldSigner: members.voter.publicKey },
          ],
          programId,
        })
      ),
        /Attempted to perform an unauthorized action/;
    });

    it("remove the member for the controlled multisig", async () => {
      const signature = await multisig.rpc.createSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda: settingsPda,
        transactionIndex: 1n,
        creator: members.proposer.publicKey,
        actions: [
          { __kind: "RemoveSigner", oldSigner: members.voter.publicKey },
        ],
        programId,
      });
      await connection.confirmTransaction(signature);
    });
  });

  describe("multisig_config_transaction_add_member", () => {
    let settingsPda: PublicKey;
    let configAuthority: Keypair;
    const newSigner = Keypair.generate();
    before(async () => {
      configAuthority = await generateFundedKeypair(connection);

      // Create new controlled multisig.
      settingsPda = (
        await createAutonomousMultisig({
          connection,
          createKey: Keypair.generate(),
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
        multisig.rpc.createSettingsTransaction({
          connection,
          feePayer,
          settingsPda: settingsPda,
          transactionIndex: 1n,
          creator: newSigner.publicKey,
          signers: [feePayer, members.proposer, newSigner],
          actions: [
            {
              __kind: "AddSigner",
              newSigner: {
                key: newSigner.publicKey,
                permissions: Permissions.all(),
              },
            },
          ],
          programId,
        })
      ),
        /Attempted to perform an unauthorized action/;
    });

    it("add member to the autonomous multisig", async () => {
      const feePayer = await generateFundedKeypair(connection);
      const signature = await multisig.rpc.createSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda: settingsPda,
        transactionIndex: 1n,
        creator: members.proposer.publicKey,
        actions: [
          {
            __kind: "AddSigner",
            newSigner: {
              key: newSigner.publicKey,
              permissions: Permissions.all(),
            },
          },
        ],
        programId,
      });
      await connection.confirmTransaction(signature);
      // create the proposal
      const createProposalSignature = await multisig.rpc.createProposal({
        connection,
        creator: members.proposer,
        settingsPda,
        feePayer,
        transactionIndex: 1n,
        isDraft: false,
        programId,
      });
      await connection.confirmTransaction(createProposalSignature);

      const approveSignature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex: 1n,
        signer: members.voter,
        programId,
      });
      await connection.confirmTransaction(approveSignature);
    });

    it("execute the add member transaction", async () => {
      const fundedKeypair = await generateFundedKeypair(connection);
      const signature = await multisig.rpc.executeSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda: settingsPda,
        transactionIndex: 1n,
        signer: members.executor,
        rentPayer: fundedKeypair,
        programId,
      });
      await connection.confirmTransaction(signature);
    });
  });

  describe("multisig_set_config_authority", () => {
    let settingsPda: PublicKey;
    let configAuthority: Keypair;
    before(async () => {
      configAuthority = await generateFundedKeypair(connection);

      // Create new controlled multisig.
      settingsPda = (
        await createControlledMultisig({
          connection,
          createKey: Keypair.generate(),
          configAuthority: configAuthority.publicKey,
          members,
          threshold: 1,
          timeLock: 0,
          programId,
        })
      )[0];
    });

    it("set `config_authority` for the controlled multisig", async () => {
      await createControlledMultisig({
        configAuthority: members.almighty.publicKey,
        members,
        connection,
        threshold: 2,
        timeLock: 0,
        createKey: Keypair.generate(),
        programId,
      });
    });
  });

  describe("multisig_add_spending_limit", () => {
    let controlledsettingsPda: PublicKey;
    let feePayer: Keypair;
    let spendingLimitPda: PublicKey;
    let spendingLimitCreateKey: PublicKey;

    before(async () => {
      controlledsettingsPda = (
        await createControlledMultisig({
          connection,
          configAuthority: members.almighty.publicKey,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];

      feePayer = await generateFundedKeypair(connection);

      spendingLimitCreateKey = Keypair.generate().publicKey;

      spendingLimitPda = multisig.getSpendingLimitPda({
        settingsPda: controlledsettingsPda,
        seed: spendingLimitCreateKey,
        programId,
      })[0];
    });

    it("error: invalid authority", async () => {
      await assert.rejects(
        () =>
          multisig.rpc.addSpendingLimitAsAuthority({
            connection,
            feePayer: feePayer,
            settingsPda: controlledsettingsPda,
            spendingLimit: spendingLimitPda,
            seed: spendingLimitCreateKey,
            rentPayer: feePayer,
            amount: BigInt(1000000000),
            settingsAuthority: members.voter,
            period: multisig.generated.Period.Day,
            mint: Keypair.generate().publicKey,
            destinations: [Keypair.generate().publicKey],
            signers: [members.almighty.publicKey],
            accountIndex: 1,
            programId,
          }),
        /Attempted to perform an unauthorized action/
      );
    });

    it("error: invalid SpendingLimit amount", async () => {
      await assert.rejects(
        () =>
          multisig.rpc.addSpendingLimitAsAuthority({
            connection,
            feePayer: feePayer,
            settingsPda: controlledsettingsPda,
            spendingLimit: spendingLimitPda,
            seed: spendingLimitCreateKey,
            rentPayer: feePayer,
            // Must be positive.
            amount: BigInt(0),
            settingsAuthority: members.almighty,
            period: multisig.generated.Period.Day,
            mint: Keypair.generate().publicKey,
            destinations: [Keypair.generate().publicKey],
            signers: [members.almighty.publicKey],
            accountIndex: 1,
            programId,
          }),
        /Invalid SpendingLimit amount/
      );
    });

    it("create a new Spending Limit for the controlled multisig with member of the ms and non-member", async () => {
      const nonMember = await generateFundedKeypair(connection);
      const expiration = Date.now() / 1000 + 5
      const signature = await multisig.rpc.addSpendingLimitAsAuthority({
        connection,
        feePayer: feePayer,
        settingsPda: controlledsettingsPda,
        spendingLimit: spendingLimitPda,
        seed: spendingLimitCreateKey,
        rentPayer: feePayer,
        amount: BigInt(1000000000),
        settingsAuthority: members.almighty,
        period: multisig.generated.Period.Day,
        mint: Keypair.generate().publicKey,
        destinations: [Keypair.generate().publicKey],
        signers: [members.almighty.publicKey, nonMember.publicKey],
        accountIndex: 1,
        expiration: expiration,
        sendOptions: { skipPreflight: true },
        programId,
      });

      await connection.confirmTransaction(signature);

      const spendingLimitAccount = await SpendingLimit.fromAccountAddress(connection, spendingLimitPda);
      assert.strictEqual(spendingLimitAccount.expiration.toString(), new BN(expiration).toString());
    });

  });

  describe("multisig_remove_spending_limit", () => {
    let controlledsettingsPda: PublicKey;
    let feePayer: Keypair;
    let spendingLimitPda: PublicKey;
    let spendingLimitCreateKey: PublicKey;

    before(async () => {
      controlledsettingsPda = (
        await createControlledMultisig({
          connection,
          configAuthority: members.almighty.publicKey,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];

      feePayer = await generateFundedKeypair(connection);

      spendingLimitCreateKey = Keypair.generate().publicKey;

      spendingLimitPda = multisig.getSpendingLimitPda({
        settingsPda: controlledsettingsPda,
        seed: spendingLimitCreateKey,
        programId,
      })[0];

      const signature = await multisig.rpc.addSpendingLimitAsAuthority({
        connection,
        feePayer: feePayer,
        settingsPda: controlledsettingsPda,
        spendingLimit: spendingLimitPda,
        seed: spendingLimitCreateKey,
        rentPayer: feePayer,
        amount: BigInt(1000000000),
        settingsAuthority: members.almighty,
        period: multisig.generated.Period.Day,
        mint: Keypair.generate().publicKey,
        destinations: [Keypair.generate().publicKey],
        signers: [members.almighty.publicKey],
        accountIndex: 1,
        sendOptions: { skipPreflight: true },
        programId,
      });

      await connection.confirmTransaction(signature);
    });

    it("error: invalid authority", async () => {
      await assert.rejects(
        () =>
          multisig.rpc.removeSpendingLimitAsAuthority({
            connection,
            settingsPda: controlledsettingsPda,
            spendingLimit: spendingLimitPda,
            settingsAuthority: members.voter.publicKey,
            feePayer: feePayer,
            rentCollector: members.voter.publicKey,
            signers: [feePayer, members.voter],
            programId,
          }),
        /Attempted to perform an unauthorized action/
      );
    });

    it("error: Spending Limit doesn't belong to the multisig", async () => {
      const wrongControlledsettingsPda = (
        await createControlledMultisig({
          connection,
          configAuthority: members.almighty.publicKey,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];

      const wrongCreateKey = Keypair.generate().publicKey;
      const wrongSpendingLimitPda = multisig.getSpendingLimitPda({
        settingsPda: wrongControlledsettingsPda,
        seed: wrongCreateKey,
        programId,
      })[0];

      const addSpendingLimitSignature =
        await multisig.rpc.addSpendingLimitAsAuthority({
          connection,
          feePayer: feePayer,
          settingsPda: wrongControlledsettingsPda,
          spendingLimit: wrongSpendingLimitPda,
          seed: wrongCreateKey,
          rentPayer: feePayer,
          amount: BigInt(1000000000),
          settingsAuthority: members.almighty,
          period: multisig.generated.Period.Day,
          mint: Keypair.generate().publicKey,
          destinations: [Keypair.generate().publicKey],
          signers: [members.almighty.publicKey],
          accountIndex: 1,
          programId,
        });

      await connection.confirmTransaction(addSpendingLimitSignature);
      await assert.rejects(
        () =>
          multisig.rpc.removeSpendingLimitAsAuthority({
            connection,
            settingsPda: controlledsettingsPda,
            spendingLimit: wrongSpendingLimitPda,
            settingsAuthority: members.almighty.publicKey,
            feePayer: feePayer,
            rentCollector: members.almighty.publicKey,
            signers: [feePayer, members.almighty],
            programId,
          }),
        /Invalid account provided/
      );
    });

    it("remove the Spending Limit from the controlled multisig", async () => {
      const signature = await multisig.rpc.removeSpendingLimitAsAuthority({
        connection,
        settingsPda: controlledsettingsPda,
        spendingLimit: spendingLimitPda,
        settingsAuthority: members.almighty.publicKey,
        feePayer: feePayer,
        rentCollector: members.almighty.publicKey,
        sendOptions: { skipPreflight: true },
        signers: [feePayer, members.almighty],
        programId,
      });
      await connection.confirmTransaction(signature);
    });
  });

  describe("config_transaction_create", () => {
    let autonomoussettingsPda: PublicKey;
    let controlledsettingsPda: PublicKey;

    before(async () => {
      // Create new autonomous multisig.
      autonomoussettingsPda = (
        await createAutonomousMultisig({
          connection,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];

      // Create new controlled multisig.
      controlledsettingsPda = (
        await createControlledMultisig({
          connection,
          configAuthority: Keypair.generate().publicKey,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];
    });

    it("error: not supported for controlled multisig", async () => {
      await assert.rejects(
        () =>
          multisig.rpc.createSettingsTransaction({
            connection,
            feePayer: members.proposer,
            settingsPda: controlledsettingsPda,
            transactionIndex: 1n,
            creator: members.proposer.publicKey,
            actions: [{ __kind: "ChangeThreshold", newThreshold: 3 }],
            programId,
          }),
        /Instruction not supported for controlled smart account/
      );
    });

    it("error: empty actions", async () => {
      await assert.rejects(
        () =>
          multisig.rpc.createSettingsTransaction({
            connection,
            feePayer: members.proposer,
            settingsPda: autonomoussettingsPda,
            transactionIndex: 1n,
            creator: members.proposer.publicKey,
            actions: [],
            programId,
          }),
        /Config transaction must have at least one action/
      );
    });

    it("error: not a member", async () => {
      const nonMember = await generateFundedKeypair(connection);

      await assert.rejects(
        () =>
          multisig.rpc.createSettingsTransaction({
            connection,
            feePayer: nonMember,
            settingsPda: autonomoussettingsPda,
            transactionIndex: 1n,
            creator: nonMember.publicKey,
            actions: [{ __kind: "ChangeThreshold", newThreshold: 3 }],
            programId,
          }),
        /Provided pubkey is not a signer of the smart account/
      );
    });

    it("error: unauthorized", async () => {
      await assert.rejects(
        () =>
          multisig.rpc.createSettingsTransaction({
            connection,
            feePayer: members.voter,
            settingsPda: autonomoussettingsPda,
            transactionIndex: 1n,
            // Voter is not authorized to initialize config transactions.
            creator: members.voter.publicKey,
            actions: [{ __kind: "ChangeThreshold", newThreshold: 3 }],
            programId,
          }),
        /Attempted to perform an unauthorized action/
      );
    });

    it("create a config transaction", async () => {
      const transactionIndex = 1n;

      const signature = await multisig.rpc.createSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda: autonomoussettingsPda,
        transactionIndex,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
        programId,
      });
      await connection.confirmTransaction(signature);

      // Fetch the multisig account.
      const multisigAccount = await Settings.fromAccountAddress(
        connection,
        autonomoussettingsPda
      );
      const lastTransactionIndex = multisig.utils.toBigInt(
        multisigAccount.transactionIndex
      );
      assert.strictEqual(lastTransactionIndex, transactionIndex);

      // Fetch the newly created ConfigTransaction account.
      const [transactionPda, transactionBump] = multisig.getTransactionPda({
        settingsPda: autonomoussettingsPda,
        transactionIndex,
        programId,
      });
      const configTransactionAccount =
        await SettingsTransaction.fromAccountAddress(connection, transactionPda);

      // Assertions.
      assert.strictEqual(
        configTransactionAccount.settings.toBase58(),
        autonomoussettingsPda.toBase58()
      );
      assert.strictEqual(
        configTransactionAccount.creator.toBase58(),
        members.proposer.publicKey.toBase58()
      );
      assert.strictEqual(
        configTransactionAccount.index.toString(),
        transactionIndex.toString()
      );
      assert.strictEqual(configTransactionAccount.bump, transactionBump);
      assert.deepEqual(configTransactionAccount.actions, [
        {
          __kind: "ChangeThreshold",
          newThreshold: 1,
        },
      ]);
    });
  });

  describe("vault_transaction_create", () => {
    let settingsPda: PublicKey;

    before(async () => {
      const msCreateKey = Keypair.generate();

      // Create new autonomous multisig.
      settingsPda = (
        await createAutonomousMultisig({
          connection,
          createKey: msCreateKey,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];
    });

    it("error: not a member", async () => {
      const nonMember = await generateFundedKeypair(connection);

      // Default vault.
      const [vaultPda] = multisig.getSmartAccountPda({
        settingsPda,
        accountIndex: 0,
        programId,
      });

      // Test transfer instruction.
      const testPayee = Keypair.generate();
      const testIx = await createTestTransferInstruction(
        vaultPda,
        testPayee.publicKey
      );
      const testTransferMessage = new TransactionMessage({
        payerKey: vaultPda,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions: [testIx],
      });

      await assert.rejects(
        () =>
          multisig.rpc.createTransaction({
            connection,
            feePayer: nonMember,
            settingsPda,
            transactionIndex: 1n,
            creator: nonMember.publicKey,
            accountIndex: 0,
            ephemeralSigners: 0,
            transactionMessage: testTransferMessage,
            programId,
          }),
        /Provided pubkey is not a signer of the smart account/
      );
    });

    it("error: unauthorized", async () => {
      // Default vault.
      const [vaultPda] = multisig.getSmartAccountPda({
        settingsPda,
        accountIndex: 0,
        programId,
      });

      // Test transfer instruction.
      const testPayee = Keypair.generate();
      const testIx = await createTestTransferInstruction(
        vaultPda,
        testPayee.publicKey
      );
      const testTransferMessage = new TransactionMessage({
        payerKey: vaultPda,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions: [testIx],
      });

      await assert.rejects(
        () =>
          multisig.rpc.createTransaction({
            connection,
            feePayer: members.voter,
            settingsPda,
            transactionIndex: 1n,
            creator: members.voter.publicKey,
            accountIndex: 0,
            ephemeralSigners: 0,
            transactionMessage: testTransferMessage,
            programId,
          }),
        /Attempted to perform an unauthorized action/
      );
    });

    it("create a new vault transaction", async () => {
      const transactionIndex = 1n;

      // Default vault.
      const [vaultPda, vaultBump] = multisig.getSmartAccountPda({
        settingsPda,
        accountIndex: 0,
        programId,
      });

      // Test transfer instruction (2x)
      const testPayee = Keypair.generate();
      const testIx1 = await createTestTransferInstruction(
        vaultPda,
        testPayee.publicKey,
        1 * LAMPORTS_PER_SOL
      );
      const testIx2 = await createTestTransferInstruction(
        vaultPda,
        testPayee.publicKey,
        1 * LAMPORTS_PER_SOL
      );
      const testTransferMessage = new TransactionMessage({
        payerKey: vaultPda,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions: [testIx1, testIx2],
      });

      const signature = await multisig.rpc.createTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        creator: members.proposer.publicKey,
        accountIndex: 0,
        ephemeralSigners: 0,
        transactionMessage: testTransferMessage,
        memo: "Transfer 2 SOL to a test account",
        programId,
      });
      await connection.confirmTransaction(signature);

      const multisigAccount = await Settings.fromAccountAddress(
        connection,
        settingsPda
      );
      assert.strictEqual(
        multisigAccount.transactionIndex.toString(),
        transactionIndex.toString()
      );

      const [transactionPda, transactionBump] = multisig.getTransactionPda({
        settingsPda,
        transactionIndex,
        programId,
      });
      const transactionAccount = await Transaction.fromAccountAddress(
        connection,
        transactionPda
      );
      assert.strictEqual(
        transactionAccount.settings.toBase58(),
        settingsPda.toBase58()
      );
      assert.strictEqual(
        transactionAccount.creator.toBase58(),
        members.proposer.publicKey.toBase58()
      );
      assert.strictEqual(
        transactionAccount.index.toString(),
        transactionIndex.toString()
      );
      assert.strictEqual(transactionAccount.accountBump, vaultBump);
      assert.deepEqual(
        transactionAccount.ephemeralSignerBumps,
        new Uint8Array()
      );
      assert.strictEqual(transactionAccount.bump, transactionBump);
      // TODO: verify the transaction message data.
      assert.ok(transactionAccount.message);
    });
  });

  describe("proposal_create", () => {
    let settingsPda: PublicKey;

    before(async () => {
      const msCreateKey = Keypair.generate();

      // Create new autonomous multisig.
      settingsPda = (
        await createAutonomousMultisig({
          connection,
          createKey: msCreateKey,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];

      // Create a config transaction.
      const newSigner = {
        key: Keypair.generate().publicKey,
        permissions: Permissions.all(),
      } as const;

      let signature = await multisig.rpc.createSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex: 1n,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "AddSigner", newSigner }],
        programId,
      });
      await connection.confirmTransaction(signature);
    });

    it("error: invalid transaction index", async () => {
      // Attempt to create a proposal for a transaction that doesn't exist.
      const transactionIndex = 2n;
      await assert.rejects(
        () =>
          multisig.rpc.createProposal({
            connection,
            feePayer: members.almighty,
            settingsPda,
            transactionIndex,
            creator: members.almighty,
            programId,
          }),
        /Invalid transaction index/
      );
    });

    it("error: non-members can't create a proposal", async () => {
      const nonMember = await generateFundedKeypair(connection);

      const transactionIndex = 2n;

      // Create a config transaction.
      let signature = await multisig.rpc.createSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
        programId,
      });
      await connection.confirmTransaction(signature);

      await assert.rejects(
        () =>
          multisig.rpc.createProposal({
            connection,
            feePayer: nonMember,
            settingsPda,
            transactionIndex,
            creator: nonMember,
            programId,
          }),
        /Provided pubkey is not a signer of the smart account/
      );
    });

    it("error: members without Initiate or Vote permissions can't create a proposal", async () => {
      const transactionIndex = 2n;

      await assert.rejects(
        () =>
          multisig.rpc.createProposal({
            connection,
            feePayer: members.executor,
            settingsPda,
            transactionIndex,
            creator: members.executor,
            programId,
          }),
        /Attempted to perform an unauthorized action/
      );
    });

    it("member with Initiate or Vote permissions can create proposal", async () => {
      const nonMember = await generateFundedKeypair(connection);

      const transactionIndex = 2n;

      // Create a proposal for the config transaction.
      let signature = await multisig.rpc.createProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex,
        creator: members.voter,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Fetch the newly created Proposal account.
      const [proposalPda, proposalBump] = multisig.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      });
      const proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );

      // Make sure the proposal was created correctly.
      assert.strictEqual(
        proposalAccount.settings.toBase58(),
        settingsPda.toBase58()
      );
      assert.strictEqual(
        proposalAccount.transactionIndex.toString(),
        transactionIndex.toString()
      );
      assert.ok(multisig.types.isProposalStatusActive(proposalAccount.status));
      assert.ok(isCloseToNow(toBigInt(proposalAccount.status.timestamp)));
      assert.strictEqual(proposalAccount.bump, proposalBump);
      assert.deepEqual(proposalAccount.approved, []);
      assert.deepEqual(proposalAccount.rejected, []);
      assert.deepEqual(proposalAccount.cancelled, []);
    });

    it("error: cannot create proposal for stale transaction", async () => {
      // Approve the second config transaction.
      let signature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex: 2n,
        signer: members.voter,
        programId,
      });
      await connection.confirmTransaction(signature);

      signature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.almighty,
        settingsPda,
        transactionIndex: 2n,
        signer: members.almighty,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Execute the second config transaction.
      signature = await multisig.rpc.executeSettingsTransaction({
        connection,
        feePayer: members.almighty,
        settingsPda,
        transactionIndex: 2n,
        signer: members.almighty,
        rentPayer: members.almighty,
        programId,
      });
      await connection.confirmTransaction(signature);

      const feePayer = await generateFundedKeypair(connection);

      // At this point the first transaction should become stale.
      // Attempt to create a proposal for it should fail.
      await assert.rejects(
        () =>
          multisig.rpc.createProposal({
            connection,
            feePayer,
            settingsPda,
            transactionIndex: 1n,
            creator: members.almighty,
            programId,
          }),
        /Proposal is stale/
      );
    });
  });

  describe("proposal_approve", () => {
    let settingsPda: PublicKey;

    before(async () => {
      const feePayer = await generateFundedKeypair(connection);
      const msCreateKey = Keypair.generate();

      // Create new autonomous multisig.
      settingsPda = (
        await createAutonomousMultisig({
          connection,
          createKey: msCreateKey,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];

      const transactionIndex = 1n;

      // Create a config transaction.
      let signature = await multisig.rpc.createSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
        programId,
      });
      await connection.confirmTransaction(signature);

      // Create a proposal for the config transaction.
      signature = await multisig.rpc.createProposal({
        connection,
        feePayer,
        settingsPda,
        transactionIndex,
        creator: members.proposer,
        programId,
      });
      await connection.confirmTransaction(signature);
    });

    it("error: not a member", async () => {
      const nonMember = await generateFundedKeypair(connection);

      const transactionIndex = 1n;

      // Non-member cannot approve the proposal.
      await assert.rejects(
        () =>
          multisig.rpc.approveProposal({
            connection,
            feePayer: nonMember,
            settingsPda,
            transactionIndex,
            signer: nonMember,
            programId,
          }),
        /Provided pubkey is not a signer of the smart account/
      );
    });

    it("error: unauthorized", async () => {
      const transactionIndex = 1n;

      // Executor is not authorized to approve config transactions.
      await assert.rejects(
        () =>
          multisig.rpc.approveProposal({
            connection,
            feePayer: members.executor,
            settingsPda,
            transactionIndex,
            signer: members.executor,
            programId,
          }),
        /Attempted to perform an unauthorized action/
      );
    });

    it("approve config transaction", async () => {
      // Approve the proposal for the first config transaction.
      const transactionIndex = 1n;

      const signature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex,
        signer: members.voter,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Fetch the Proposal account.
      const [proposalPda] = multisig.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      });
      const proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );

      // Assertions.
      assert.deepEqual(proposalAccount.approved, [members.voter.publicKey]);
      assert.deepEqual(proposalAccount.rejected, []);
      assert.deepEqual(proposalAccount.cancelled, []);
      // Our threshold is 2, so the proposal is not yet Approved.
      assert.ok(multisig.types.isProposalStatusActive(proposalAccount.status));
    });

    it("error: already approved", async () => {
      // Approve the proposal for the first config transaction once again.
      const transactionIndex = 1n;

      await assert.rejects(
        () =>
          multisig.rpc.approveProposal({
            connection,
            feePayer: members.voter,
            settingsPda,
            transactionIndex,
            signer: members.voter,
            programId,
          }),
        /Signer already approved the transaction/
      );
    });

    it("approve config transaction and reach threshold", async () => {
      // Approve the proposal for the first config transaction.
      const transactionIndex = 1n;

      const signature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.almighty,
        settingsPda,
        transactionIndex,
        signer: members.almighty,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Fetch the Proposal account.
      const [proposalPda] = multisig.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      });
      const proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );

      // Assertions.
      assert.deepEqual(
        proposalAccount.approved.map((key) => key.toBase58()),
        [members.voter.publicKey, members.almighty.publicKey]
          .sort(comparePubkeys)
          .map((key) => key.toBase58())
      );
      assert.deepEqual(proposalAccount.rejected, []);
      assert.deepEqual(proposalAccount.cancelled, []);
      // Our threshold is 2, so the transaction is now Approved.
      assert.ok(
        multisig.types.isProposalStatusApproved(proposalAccount.status)
      );
    });

    it("error: stale transaction");

    it("error: invalid transaction status");

    it("error: proposal is not for multisig");
  });

  describe("proposal_reject", () => {
    let settingsPda: PublicKey;

    before(async () => {
      const feePayer = await generateFundedKeypair(connection);
      const msCreateKey = Keypair.generate();

      // Create new autonomous multisig.
      settingsPda = (
        await createAutonomousMultisig({
          connection,
          createKey: msCreateKey,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];

      // Create first config transaction.
      let signature = await multisig.rpc.createSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex: 1n,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
        programId,
      });
      await connection.confirmTransaction(signature);

      // Create second config transaction.
      signature = await multisig.rpc.createSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex: 2n,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "SetTimeLock", newTimeLock: 60 }],
        programId,
      });
      await connection.confirmTransaction(signature);

      // Create a proposal for the first config transaction.
      signature = await multisig.rpc.createProposal({
        connection,
        feePayer,
        settingsPda,
        transactionIndex: 1n,
        creator: members.proposer,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Create a proposal for the second config transaction.
      signature = await multisig.rpc.createProposal({
        connection,
        feePayer,
        settingsPda,
        transactionIndex: 2n,
        creator: members.proposer,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Approve the proposal for the first config transaction and reach the threshold.
      signature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex: 1n,
        signer: members.voter,
        programId,
      });
      await connection.confirmTransaction(signature);
      signature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.almighty,
        settingsPda,
        transactionIndex: 1n,
        signer: members.almighty,
        programId,
      });
      await connection.confirmTransaction(signature);
    });

    it("error: try to reject an approved proposal", async () => {
      // Reject the proposal for the first config transaction.
      const transactionIndex = 1n;

      await assert.rejects(
        () =>
          multisig.rpc.rejectProposal({
            connection,
            feePayer: members.voter,
            settingsPda,
            transactionIndex,
            signer: members.voter,
            programId,
          }),
        /Invalid proposal status/
      );
      const proposalAccount = await Proposal.fromAccountAddress(
        connection,
        multisig.getProposalPda({
          settingsPda,
          transactionIndex,
          programId,
        })[0]
      );
      assert.ok(
        multisig.types.isProposalStatusApproved(proposalAccount.status)
      );
    });

    it("error: not a member", async () => {
      const nonMember = await generateFundedKeypair(connection);

      // Reject the proposal for the second config transaction.
      const transactionIndex = 2n;

      await assert.rejects(
        () =>
          multisig.rpc.rejectProposal({
            connection,
            feePayer: nonMember,
            settingsPda,
            transactionIndex,
            signer: nonMember,
            programId,
          }),
        /Provided pubkey is not a signer of the smart account/
      );
    });

    it("error: unauthorized", async () => {
      // Reject the proposal for the second config transaction.
      const transactionIndex = 2n;

      await assert.rejects(
        () =>
          multisig.rpc.rejectProposal({
            connection,
            feePayer: members.executor,
            settingsPda,
            transactionIndex,
            signer: members.executor,
            programId,
          }),
        /Attempted to perform an unauthorized action/
      );
    });

    it("reject proposal and reach cutoff", async () => {
      let multisigAccount = await Settings.fromAccountAddress(
        connection,
        settingsPda
      );

      // Reject the proposal for the second config transaction.
      const transactionIndex = 2n;

      const signature = await multisig.rpc.rejectProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex,
        signer: members.voter,
        memo: "LGTM",
        programId,
      });
      await connection.confirmTransaction(signature);

      // Fetch the Proposal account.
      const [proposalPda] = multisig.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      });
      const proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );
      assert.deepEqual(proposalAccount.approved, []);
      assert.deepEqual(proposalAccount.rejected, [members.voter.publicKey]);
      assert.deepEqual(proposalAccount.cancelled, []);
      // Our threshold is 2, and 2 voters, so the cutoff is 1...
      assert.strictEqual(multisigAccount.threshold, 2);
      assert.strictEqual(
        multisigAccount.signers.filter((m) =>
          Permissions.has(m.permissions, Permission.Vote)
        ).length,
        2
      );
      // ...thus we've reached the cutoff, and the proposal is now Rejected.
      assert.ok(
        multisig.types.isProposalStatusRejected(proposalAccount.status)
      );
    });

    it("error: already rejected", async () => {
      // Reject the proposal for the second config transaction.
      const transactionIndex = 2n;

      await assert.rejects(
        () =>
          multisig.rpc.rejectProposal({
            connection,
            feePayer: members.almighty,
            settingsPda,
            transactionIndex,
            signer: members.almighty,
            programId,
          }),
        /Invalid proposal status/
      );

      const proposalAccount = await Proposal.fromAccountAddress(
        connection,
        multisig.getProposalPda({
          settingsPda,
          transactionIndex,
          programId,
        })[0]
      );
      assert.ok(
        multisig.types.isProposalStatusRejected(proposalAccount.status)
      );
    });

    it("error: stale transaction");

    it("error: transaction is not for multisig");
  });

  describe("proposal_cancel", () => {
    let settingsPda: PublicKey;

    before(async () => {
      const feePayer = await generateFundedKeypair(connection);
      const msCreateKey = Keypair.generate();

      // Create new autonomous multisig.
      settingsPda = (
        await createAutonomousMultisig({
          connection,
          createKey: msCreateKey,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];

      // Create a config transaction.
      let signature = await multisig.rpc.createSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex: 1n,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
        programId,
      });
      await connection.confirmTransaction(signature);

      // Create a proposal for the config transaction.
      signature = await multisig.rpc.createProposal({
        connection,
        feePayer,
        settingsPda,
        transactionIndex: 1n,
        creator: members.proposer,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Approve the proposal for the config transaction and reach the threshold.
      signature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex: 1n,
        signer: members.voter,
        programId,
      });
      await connection.confirmTransaction(signature);
      signature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.almighty,
        settingsPda,
        transactionIndex: 1n,
        signer: members.almighty,
        programId,
      });
      await connection.confirmTransaction(signature);

      // The proposal must be `Approved` now.
      const [proposalPda] = multisig.getProposalPda({
        settingsPda,
        transactionIndex: 1n,
        programId,
      });
      let proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );
      assert.ok(
        multisig.types.isProposalStatusApproved(proposalAccount.status)
      );
    });

    it("cancel proposal", async () => {
      const transactionIndex = 1n;

      // Now cancel the proposal.
      let signature = await multisig.rpc.cancelProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex,
        signer: members.voter,
        programId,
      });
      await connection.confirmTransaction(signature);

      const proposalPda = multisig.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      })[0];
      let proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );
      // Our threshold is 2, so after the first cancel, the proposal is still `Approved`.
      assert.ok(
        multisig.types.isProposalStatusApproved(proposalAccount.status)
      );

      // Second member cancels the transaction.
      signature = await multisig.rpc.cancelProposal({
        connection,
        feePayer: members.almighty,
        settingsPda,
        transactionIndex,
        signer: members.almighty,
        programId,
      });
      await connection.confirmTransaction(signature);

      proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );
      // Reached the threshold, so the transaction should be `Cancelled` now.
      assert.ok(
        multisig.types.isProposalStatusCancelled(proposalAccount.status)
      );
    });

    it("proposal_cancel_v2", async () => {
      // Create a config transaction.
      const transactionIndex = 2n;
      let newVotingMember = new Keypair();

      const [proposalPda] = multisig.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      });

      let signature = await multisig.rpc.createSettingsTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        creator: members.proposer.publicKey,
        actions: [{ __kind: "AddSigner", newSigner: { key: newVotingMember.publicKey, permissions: multisig.types.Permissions.all() } }],
        programId,
      });
      await connection.confirmTransaction(signature);

      // Create a proposal for the transaction.
      signature = await multisig.rpc.createProposal({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        creator: members.proposer,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Approve the proposal 1.
      signature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex,
        signer: members.voter,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Approve the proposal 2.
      signature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.almighty,
        settingsPda,
        transactionIndex,
        signer: members.almighty,
        programId,
      });
      await connection.confirmTransaction(signature);

      let proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );
      // Our threshold is 2, so after the first cancel, the proposal is still `Approved`.
      assert.ok(
        multisig.types.isProposalStatusApproved(proposalAccount.status)
      );

      // Proposal is now ready to execute, cast the 2 cancels using the new functionality.
      signature = await multisig.rpc.cancelProposal({
        connection,
        feePayer: members.voter,
        signer: members.voter,
        settingsPda,
        transactionIndex,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Proposal is now ready to execute, cast the 2 cancels using the new functionality.
      signature = await multisig.rpc.cancelProposal({
        connection,
        feePayer: members.almighty,
        signer: members.almighty,
        settingsPda,
        transactionIndex,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Proposal status must be "Cancelled".
      proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );
      assert.ok(multisig.types.isProposalStatusCancelled(proposalAccount.status));
    });

  });

  describe("vault_transaction_execute", () => {
    let settingsPda: PublicKey;

    before(async () => {
      // Create new autonomous multisig.
      settingsPda = (
        await createAutonomousMultisig({
          connection,
          members,
          threshold: 2,
          timeLock: 0,
          programId,
        })
      )[0];

      // Default vault.
      const [vaultPda, vaultBump] = multisig.getSmartAccountPda({
        settingsPda,
        accountIndex: 0,
        programId,
      });

      // Airdrop 2 SOL to the Vault, we'll need it for the test transfer instructions.
      const airdropSig = await connection.requestAirdrop(
        vaultPda,
        2 * LAMPORTS_PER_SOL
      );
      await connection.confirmTransaction(airdropSig);

      // Test transfer instruction (2x)
      const testPayee = Keypair.generate();
      const testIx1 = await createTestTransferInstruction(
        vaultPda,
        testPayee.publicKey,
        1 * LAMPORTS_PER_SOL
      );
      const testIx2 = await createTestTransferInstruction(
        vaultPda,
        testPayee.publicKey,
        1 * LAMPORTS_PER_SOL
      );
      const testTransferMessage = new TransactionMessage({
        payerKey: vaultPda,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions: [testIx1, testIx2],
      });

      const transactionIndex = 1n;

      // Create a vault transaction.
      let signature = await multisig.rpc.createTransaction({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        creator: members.proposer.publicKey,
        accountIndex: 0,
        ephemeralSigners: 0,
        transactionMessage: testTransferMessage,
        memo: "Transfer 2 SOL to a test account",
        programId,
      });
      await connection.confirmTransaction(signature);

      // Create a proposal for the transaction.
      signature = await multisig.rpc.createProposal({
        connection,
        feePayer: members.proposer,
        settingsPda,
        transactionIndex,
        creator: members.proposer,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Approve the proposal by the first member.
      signature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.voter,
        settingsPda,
        transactionIndex,
        signer: members.voter,
        programId,
      });
      await connection.confirmTransaction(signature);

      // Approve the proposal by the second member.
      signature = await multisig.rpc.approveProposal({
        connection,
        feePayer: members.almighty,
        settingsPda,
        transactionIndex,
        signer: members.almighty,
        programId,
      });
      await connection.confirmTransaction(signature);
    });

    it("execute a vault transaction", async () => {
      // Execute the vault transaction.
      const transactionIndex = 1n;

      const [transactionPda] = multisig.getTransactionPda({
        settingsPda,
        transactionIndex,
        programId,
      });
      let transactionAccount = await Transaction.fromAccountAddress(
        connection,
        transactionPda
      );

      const [proposalPda] = multisig.getProposalPda({
        settingsPda,
        transactionIndex,
        programId,
      });

      const [vaultPda] = multisig.getSmartAccountPda({
        settingsPda,
        accountIndex: transactionAccount.accountIndex,
        programId,
      });
      const preVaultBalance = await connection.getBalance(vaultPda);
      assert.strictEqual(preVaultBalance, 2 * LAMPORTS_PER_SOL);

      // Execute the transaction.
      const signature = await multisig.rpc.executeTransaction({
        connection,
        feePayer: members.executor,
        settingsPda,
        transactionIndex,
        signer: members.executor.publicKey,
        signers: [members.executor],
        programId,
      });
      await connection.confirmTransaction(signature);

      // Verify the transaction account.
      const proposalAccount = await Proposal.fromAccountAddress(
        connection,
        proposalPda
      );
      assert.ok(
        multisig.types.isProposalStatusExecuted(proposalAccount.status)
      );

      const postVaultBalance = await connection.getBalance(vaultPda);
      // Transferred 2 SOL to payee.
      assert.strictEqual(postVaultBalance, 0);
    });

    it("error: not a member");

    it("error: unauthorized");

    it("error: invalid transaction status");

    it("error: transaction is not for multisig");

    it("error: execute reentrancy");
  });

  describe("utils", () => {
    describe("getAvailableMemoSize", () => {
      it("provides estimates for available size to use for memo", async () => {
        const multisigCreator = await generateFundedKeypair(connection);
        const createKey = Keypair.generate();
        const [settingsPda] = multisig.getSettingsPda({
          createKey: createKey.publicKey,
          programId,
        });
        const [configAuthority] = multisig.getSmartAccountPda({
          settingsPda,
          accountIndex: 0,
          programId,
        });
        const programConfigPda = multisig.getProgramConfigPda({ programId })[0];
        const programConfig = await multisig.accounts.ProgramConfig.fromAccountAddress(
          connection,
          programConfigPda
        );
        const treasury = programConfig.treasury;
        const multisigCreateArgs: Parameters<
          typeof multisig.transactions.createSmartAccount
        >[0] = {
          blockhash: (await connection.getLatestBlockhash()).blockhash,
          createKey: createKey.publicKey,
          creator: multisigCreator.publicKey,
          treasury: treasury,
          rentCollector: null,
          settings: settingsPda,
          settingsAuthority: configAuthority,
          timeLock: 0,
          signers: [
            {
              key: members.almighty.publicKey,
              permissions: Permissions.all(),
            },
          ],
          threshold: 1,
          programId,
        };

        const createMultisigTxWithoutMemo =
          multisig.transactions.createSmartAccount(multisigCreateArgs);

        const availableMemoSize = multisig.utils.getAvailableMemoSize(
          createMultisigTxWithoutMemo
        );

        const memo = "a".repeat(availableMemoSize);

        const createMultisigTxWithMemo = multisig.transactions.createSmartAccount({
          ...multisigCreateArgs,
          memo,
        });
        // The transaction with memo should have the maximum allowed size.
        assert.strictEqual(createMultisigTxWithMemo.serialize().length, 1232);
        // The transaction should work.
        createMultisigTxWithMemo.sign([multisigCreator, createKey]);
        const signature = await connection.sendTransaction(
          createMultisigTxWithMemo
        );
        await connection.confirmTransaction(signature);
      });
    });
  });
});
