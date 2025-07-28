import * as smartAccount from "@sqds/smart-account";
import * as web3 from "@solana/web3.js";
import assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  generateSmartAccountSigners,
  getTestProgramId,
  TestMembers,
} from "../../utils";
const { Settings, Proposal, Policy } = smartAccount.accounts;
const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Flow / SettingsChangePolicy", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("create policy: SettingsChange", async () => {
    // Create new autonomous smart account with 1/1 threshold
    const settingsPda = (
      await createAutonomousMultisig({
        connection,
        members,
        threshold: 1,
        timeLock: 0,
        programId,
      })
    )[0];

    const policySeed = 1;

    let allowedKeypair = web3.Keypair.generate();
    const policyCreationPayload: smartAccount.generated.PolicyCreationPayload =
      {
        __kind: "SettingsChange",
        fields: [
          {
            actions: [
              {
                __kind: "AddSigner",
                newSigner: allowedKeypair.publicKey,
                newSignerPermissions: {
                  mask: 1, // Allow only voting
                },
              },
              { __kind: "ChangeThreshold" },
            ], // Allow threshold changes
          },
        ],
      };

    const transactionIndex = BigInt(1);

    const [policyPda] = smartAccount.getPolicyPda({
      settingsPda,
      policySeed,
      programId,
    });

    let signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      actions: [
        {
          __kind: "PolicyCreate",
          seed: policySeed,
          policyCreationPayload,
          signers: [
            {
              key: members.voter.publicKey,
              permissions: { mask: 7 },
            },
          ],
          threshold: 1,
          timeLock: 0,
          startTimestamp: Date.now(),
          expirationArgs: null,
        },
      ],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create and approve proposal
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Execute settings transaction
    signature = await smartAccount.rpc.executeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      rentPayer: members.almighty,
      policies: [policyPda],
      sendOptions: { skipPreflight: true },
      programId,
    });
    await connection.confirmTransaction(signature);

    // Check settings counter incremented
    const settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settingsAccount.policySeed?.toString(), "1");

    const policyAccount = await Policy.fromAccountAddress(
      connection,
      policyPda
    );
    assert.strictEqual(
      policyAccount.settings.toString(),
      settingsPda.toString()
    );
    assert.strictEqual(policyAccount.threshold, 1);

    const instructionAccounts = [
      {
        pubkey: members.voter.publicKey,
        isWritable: false,
        isSigner: true,
      },
      {
        pubkey: settingsPda,
        isWritable: true,
        isSigner: false,
      },
      // Rent payer
      {
        pubkey: members.voter.publicKey,
        isWritable: true,
        isSigner: true,
      },

      // System program
      {
        pubkey: web3.SystemProgram.programId,
        isWritable: false,
        isSigner: false,
      },
    ];
    // Try to add the new signer to the policy
    signature = await smartAccount.rpc.executePolicyPayloadSync({
      connection,
      feePayer: members.almighty,
      policy: policyPda,
      accountIndex: 0,
      numSigners: 1,
      signers: [members.voter],
      programId,
      policyPayload: {
        __kind: "SettingsChange",
        fields: [
          {
            actionIndex: new Uint8Array([0]),
            actions: [
              {
                __kind: "AddSigner",
                newSigner: {
                  key: allowedKeypair.publicKey,
                  permissions: { mask: 1 },
                },
              },
            ],
          },
        ],
      },
      instruction_accounts: instructionAccounts,
    });

    // Wrong action index
    await assert.rejects(
      smartAccount.rpc.executePolicyPayloadSync({
        connection,
        feePayer: members.almighty,
        policy: policyPda,
        accountIndex: 0,
        numSigners: 1,
        signers: [members.voter],
        programId,
        policyPayload: {
          __kind: "SettingsChange",
          fields: [
            {
              // Change threshold
              actionIndex: new Uint8Array([1]),
              actions: [
                {
                  __kind: "AddSigner",
                  newSigner: {
                    key: allowedKeypair.publicKey,
                    permissions: { mask: 1 },
                  },
                },
              ],
            },
          ],
        },
        instruction_accounts: instructionAccounts,
      }),
      (error: any) => {
        error.toString().includes("SettingsChangeActionMismatch");
        return true;
      }
    );

    // Wrong action index
    await assert.rejects(
      smartAccount.rpc.executePolicyPayloadSync({
        connection,
        feePayer: members.almighty,
        policy: policyPda,
        accountIndex: 0,
        numSigners: 1,
        signers: [members.voter],
        programId,
        policyPayload: {
          __kind: "SettingsChange",
          fields: [
            {
              // Change threshold
              actionIndex: new Uint8Array([0]),
              actions: [
                {
                  __kind: "AddSigner",
                  newSigner: {
                    key: allowedKeypair.publicKey,
                    permissions: { mask: 7 },
                  },
                },
              ],
            },
          ],
        },
        instruction_accounts: instructionAccounts,
      }),
      (error: any) => {
        error
          .toString()
          .includes("SettingsChangeAddSignerPermissionsViolation");
        return true;
      }
    );
  });
});
