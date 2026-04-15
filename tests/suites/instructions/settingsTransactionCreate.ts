import { Keypair, PublicKey } from "@solana/web3.js";
import * as smartAccount from "@sqds/smart-account";
import * as assert from "assert";
import {
  createAutonomousMultisig,
  createControlledSmartAccount,
  createLocalhostConnection,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getNextAccountIndex,
  getTestProgramId,
  TestMembers,
  createSignerArray,
  createSignerObject,
  formatsToRun,
  getRpc,
} from "../../utils";

const { toBigInt } = smartAccount.utils;
const { Settings, SettingsTransaction, Proposal } = smartAccount.accounts;
const { Permission, Permissions } = smartAccount.types;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

for (const format of formatsToRun) {
  const rpc = getRpc(format);

  describe(`Instructions / settings_transaction_create [${format}]`, () => {
    let members: TestMembers;

    before(async () => {
      members = await generateSmartAccountSigners(connection);
    });

    describe("settings_transaction_create", () => {
      let autonomoussettingsPda: PublicKey;
      let controlledsettingsPda: PublicKey;

      before(async () => {
        // Create new autonomous smartAccount.
        autonomoussettingsPda = (
          await createAutonomousMultisig({
            connection,
            members,
            threshold: 2,
            timeLock: 0,
            programId,
          })
        )[0];
        const accountIndex = await getNextAccountIndex(connection, programId);
        // Create new controlled smartAccount.
        controlledsettingsPda = (
          await createControlledSmartAccount({
            accountIndex,
            connection,
            configAuthority: Keypair.generate().publicKey,
            members,
            threshold: 2,
            timeLock: 0,
            programId,
          })
        )[0];
      });

      it("error: not supported for controlled smart account", async () => {
        await assert.rejects(
          () =>
            rpc.createSettingsTransaction({
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
            rpc.createSettingsTransaction({
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
            rpc.createSettingsTransaction({
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
            rpc.createSettingsTransaction({
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

      it("create a settings transaction", async () => {
        const transactionIndex = 1n;

        const signature = await rpc.createSettingsTransaction({
          connection,
          feePayer: members.proposer,
          settingsPda: autonomoussettingsPda,
          transactionIndex,
          creator: members.proposer.publicKey,
          actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
          programId,
        });
        await connection.confirmTransaction(signature);

        // Fetch the smart account account.
        const multisigAccount = await Settings.fromAccountAddress(
          connection,
          autonomoussettingsPda
        );
        const lastTransactionIndex = smartAccount.utils.toBigInt(
          multisigAccount.transactionIndex
        );
        assert.strictEqual(lastTransactionIndex, transactionIndex);

        // Fetch the newly created ConfigTransaction account.
        const [transactionPda, transactionBump] =
          smartAccount.getTransactionPda({
            settingsPda: autonomoussettingsPda,
            transactionIndex,
            programId,
          });
        const configTransactionAccount =
          await SettingsTransaction.fromAccountAddress(
            connection,
            transactionPda
          );

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

    describe("smart_account_settings_transaction_remove_signer", () => {
      let settingsPda: PublicKey;
      let configAuthority: Keypair;
      before(async () => {
        configAuthority = await generateFundedKeypair(connection);
        const accountIndex = await getNextAccountIndex(connection, programId);
        // Create new controlled smartAccount.
        settingsPda = (
          await createAutonomousMultisig({
            connection,
            accountIndex,
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
          rpc.createSettingsTransaction({
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

      it("remove the signer for the controlled smart account", async () => {
        const signature = await rpc.createSettingsTransaction({
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

    describe("smart_account_settings_transaction_add_signer", () => {
      let settingsPda: PublicKey;
      let configAuthority: Keypair;
      const newSigner = Keypair.generate();
      before(async () => {
        configAuthority = await generateFundedKeypair(connection);
        const accountIndex = await getNextAccountIndex(connection, programId);
        // Create new controlled smartAccount.
        settingsPda = (
          await createAutonomousMultisig({
            connection,
            accountIndex,
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
          rpc.createSettingsTransaction({
            connection,
            feePayer,
            settingsPda: settingsPda,
            transactionIndex: 1n,
            creator: newSigner.publicKey,
            signers: [feePayer, members.proposer, newSigner],
            actions: [
              {
                __kind: "AddSigner",
                newSigner: createSignerArray(
                  newSigner.publicKey,
                  Permissions.all()
                ),
              },
            ],
            programId,
          })
        ),
          /Attempted to perform an unauthorized action/;
      });

      it("add signer to the autonomous smart account", async () => {
        const feePayer = await generateFundedKeypair(connection);
        const signature = await rpc.createSettingsTransaction({
          connection,
          feePayer: members.proposer,
          settingsPda: settingsPda,
          transactionIndex: 1n,
          creator: members.proposer.publicKey,
          actions: [
            {
              __kind: "AddSigner",
              newSigner: createSignerArray(
                newSigner.publicKey,
                Permissions.all()
              ),
            },
          ],
          programId,
        });
        await connection.confirmTransaction(signature);
        // create the proposal
        const createProposalSignature = await rpc.createProposal({
          connection,
          creator: members.proposer,
          settingsPda,
          feePayer,
          transactionIndex: 1n,
          isDraft: false,
          programId,
        });
        await connection.confirmTransaction(createProposalSignature);

        const approveSignature = await rpc.approveProposal({
          connection,
          feePayer: members.voter,
          settingsPda,
          transactionIndex: 1n,
          signer: members.voter,
          programId,
        });
        await connection.confirmTransaction(approveSignature);
      });

      it("execute the add signer transaction", async () => {
        const fundedKeypair = await generateFundedKeypair(connection);
        const signature = await rpc.executeSettingsTransaction({
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
  });
}
