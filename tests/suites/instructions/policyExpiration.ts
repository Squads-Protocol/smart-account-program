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

describe("Flows / Policy Expiration", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("Test: Policy State Expiry", async () => {
    // Create new autonomous smart account with 1/1 threshold for easy testing
    const settingsPda = (
      await createAutonomousMultisig({
        connection,
        members,
        threshold: 1,
        timeLock: 0,
        programId,
      })
    )[0];

    // Use seed 1 for the first policy on this smart account
    const policySeed = 1;

    // Create policy creation payload
    const policyCreationPayload: smartAccount.generated.PolicyCreationPayload =
      {
        __kind: "InternalFundTransfer",
        fields: [
          {
            sourceAccountIndices: new Uint8Array([0, 1]), // Allow transfers from account indices 0 and 1
            destinationAccountIndices: new Uint8Array([2, 3]), // Allow transfers to account indices 2 and 3
            allowedMints: [web3.PublicKey.default], // Allow native SOL transfers
          },
        ],
      };

    const transactionIndex = BigInt(1);

    const [policyPda] = smartAccount.getPolicyPda({
      settingsPda,
      policySeed,
      programId,
    });
    // Create settings transaction with PolicyCreate action
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
          startTimestamp: null,
          expirationArgs: {
            __kind: "SettingsState",
          },
        },
      ],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create proposal for the transaction
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal (1/1 threshold)
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Execute the settings transaction
    signature = await smartAccount.rpc.executeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      rentPayer: members.almighty,
      policies: [policyPda],
      programId,
      sendOptions: {
        skipPreflight: true,
      },
    });
    await connection.confirmTransaction(signature);
    // Check settings counter incremented
    const settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settingsAccount.policySeed?.toString(), "1");

    // Add a member to the smart account settings
    signature = await smartAccount.rpc.executeSettingsTransactionSync({
      connection,
      feePayer: members.proposer,
      settingsPda,
      actions: [
        {
          __kind: "AddSigner",
          newSigner: {
            key: web3.PublicKey.unique(),
            permissions: { mask: 7 },
          },
        },
      ],
      signers: [members.almighty],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Assert the policy is expired due to settings state changing
    const policyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "InternalFundTransfer",
      fields: [
        {
          sourceIndex: 0,
          destinationIndex: 2,
          mint: web3.PublicKey.default,
          decimals: 9,
          // 1 SOL
          amount: 1_000_000_000,
        },
      ],
    };

    // Reject due to lack of settings account submission
    assert.rejects(
      async () => {
        await smartAccount.rpc.executePolicyPayloadSync({
          connection,
          feePayer: members.voter,
          policy: policyPda,
          accountIndex: 0,
          policyPayload,
          numSigners: 1,
          // Not submitting the settings account is a violation
          instruction_accounts: [],
          signers: [members.voter],
          programId,
        });
      },
      (err: any) => {
        assert.ok(
          err
            .toString()
            .includes("PolicyExpirationViolationSettingsAccountNotPresent")
        );
        return true;
      }
    );

    // Reject due to mismatching settings account submission
    assert.rejects(
      async () => {
        await smartAccount.rpc.executePolicyPayloadSync({
          connection,
          feePayer: members.voter,
          policy: policyPda,
          accountIndex: 0,
          policyPayload,
          numSigners: 1,
          // Not submitting the settings account is a violation
          instruction_accounts: [
            // Passing a random account as remaining account 0
            {
              pubkey: members.proposer.publicKey,
              isWritable: true,
              isSigner: false,
            },
          ],
          signers: [members.voter],
          programId,
        });
      },
      (err: any) => {
        assert.ok(
          err
            .toString()
            .includes("PolicyExpirationViolationPolicySettingsKeyMismatch")
        );
        return true;
      }
    );

    // Reject due to settings hash expiration
    assert.rejects(
      async () => {
        await smartAccount.rpc.executePolicyPayloadSync({
          connection,
          feePayer: members.voter,
          policy: policyPda,
          accountIndex: 0,
          policyPayload,
          numSigners: 1,
          // Not submitting the settings account is a violation
          instruction_accounts: [
            // Passing a random account as remaining account 0
            {
              pubkey: settingsPda,
              isWritable: false,
              isSigner: false,
            },
          ],
          signers: [members.voter],
          programId,
        });
      },
      (err: any) => {
        assert.ok(
          err.toString().includes("PolicyExpirationViolationHashExpired")
        );
        return true;
      }
    );

    // Update the policy to use the new settings hash
    signature = await smartAccount.rpc.executeSettingsTransactionSync({
      connection,
      feePayer: members.almighty,
      settingsPda,
      actions: [
        {
          __kind: "PolicyUpdate",
          policy: policyPda,
          signers: [
            {
              key: members.voter.publicKey,
              permissions: { mask: 7 },
            },
          ],
          threshold: 1,
          timeLock: 0,
          policyUpdatePayload: policyCreationPayload,
          expirationArgs: {
            __kind: "SettingsState",
          },
        },
      ],
      remainingAccounts: [
        {
          pubkey: policyPda,
          isWritable: true,
          isSigner: false,
        },
      ],
      signers: [members.almighty],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Get the destination and source accounts
    let [destinationAccount] = await smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 2,
      programId,
    });
    let [sourceAccount] = await smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    // Airdrop SOL to the source account
    let airdropSignature = await connection.requestAirdrop(
      sourceAccount,
      1_000_000_000
    );
    await connection.confirmTransaction(airdropSignature);

    // Execute the policy payload
    signature = await smartAccount.rpc.executePolicyPayloadSync({
      connection,
      feePayer: members.voter,
      policy: policyPda,
      accountIndex: 0,
      policyPayload,
      numSigners: 1,
      instruction_accounts: [
        {
          pubkey: members.voter.publicKey,
          isWritable: false,
          isSigner: true,
        },
        {
          pubkey: settingsPda,
          isWritable: false,
          isSigner: false,
        },
        {
          pubkey: sourceAccount,
          isWritable: true,
          isSigner: false,
        },
        {
          pubkey: destinationAccount,
          isWritable: true,
          isSigner: false,
        },
        {
          pubkey: web3.SystemProgram.programId,
          isWritable: false,
          isSigner: false,
        },
      ],
      signers: [members.voter],
      sendOptions: {
        skipPreflight: true,
      },
      programId,
    });
    await connection.confirmTransaction(signature);

    // Check the balances
    let destinationBalance = await connection.getBalance(destinationAccount);
    assert.strictEqual(destinationBalance, 1_000_000_000);
  });
});
