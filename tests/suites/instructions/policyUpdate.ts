import * as smartAccount from "@sqds/smart-account";
import * as web3 from "@solana/web3.js";
import assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  generateSmartAccountSigners,
  getTestProgramId,
  TestMembers, 
  createSignerObject,
  formatsToRun,
  getRpc,
} from "../../utils";
import { AccountMeta } from "@solana/web3.js";
const { Settings, Proposal, Policy } = smartAccount.accounts;
const programId = getTestProgramId();
const connection = createLocalhostConnection();

for (const format of formatsToRun) {
  const rpc = getRpc(format);

  describe(`Flows / Policy Update [${format}]`, () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("update policy: InternalFundTransfer", async () => {
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

    // Increment account_utilization to unlock indices 1, 2, 3 (test uses 0-3)
    for (let i = 0; i < 3; i++) {
      const ix = smartAccount.generated.createIncrementAccountIndexInstruction(
        { settings: settingsPda, signer: members.almighty.publicKey, program: programId },
        programId
      );
      const msg = new web3.TransactionMessage({
        payerKey: members.almighty.publicKey,
        recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
        instructions: [ix],
      }).compileToV0Message();
      const tx = new web3.VersionedTransaction(msg);
      tx.sign([members.almighty]);
      await connection.confirmTransaction(
        await connection.sendRawTransaction(tx.serialize())
      );
    }

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
    let signature = await rpc.createSettingsTransaction({
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
          expirationArgs: null,
        },
      ],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create proposal for the transaction
    signature = await rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal (1/1 threshold)
    signature = await rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Execute the settings transaction
    signature = await rpc.executeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      rentPayer: members.almighty,
      policies: [policyPda],
      programId,
    });
    await connection.confirmTransaction(signature);
    // Check settings counter incremented
    const settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settingsAccount.policySeed?.toString(), "1");

    // Create a proposal for the policy
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
    // Create a transaction
    signature = await rpc.createPolicyTransaction({
      connection,
      feePayer: members.voter,
      policy: policyPda,
      accountIndex: 0,
      transactionIndex: BigInt(1),
      creator: members.voter.publicKey,
      policyPayload,
      sendOptions: {
        skipPreflight: true,
      },
      programId,
    });
    await connection.confirmTransaction(signature);
    // Assert the policies tx index increased
    let policyAccount = await Policy.fromAccountAddress(connection, policyPda);
    assert.strictEqual(policyAccount.transactionIndex.toString(), "1");
    assert.strictEqual(policyAccount.staleTransactionIndex?.toString(), "0");

    // Update the policy
    let remainingAccounts: AccountMeta[] = [];
    remainingAccounts.push({
      pubkey: policyPda,
      isWritable: true,
      isSigner: false,
    });

    let updateSignature = await rpc.executeSettingsTransactionSync(
      {
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
            policyUpdatePayload: {
              __kind: "InternalFundTransfer",
              fields: [
                {
                  sourceAccountIndices: new Uint8Array([0, 1]), // Allow transfers from account indices 0 and 1
                  destinationAccountIndices: new Uint8Array([2, 3]), // Allow transfers to account indices 2 and 3
                  allowedMints: [members.voter.publicKey], // Change the mint
                },
              ],
            },
            expirationArgs: null,
          },
        ],
        signers: [members.almighty],
        remainingAccounts,
        programId,
      }
    );
    await connection.confirmTransaction(updateSignature);

    // Check the policy stale tx index increased
    policyAccount = await Policy.fromAccountAddress(connection, policyPda);
    assert.strictEqual(policyAccount.transactionIndex.toString(), "1");
    assert.strictEqual(policyAccount.staleTransactionIndex?.toString(), "1");

    // Check the policy state updated
    let policyState = policyAccount.policyState;
    let programInteractionPolicy = policyState
      .fields[0] as smartAccount.generated.InternalFundTransferPolicy;
    let allowedMints = programInteractionPolicy.allowedMints;
    assert.equal(
      allowedMints[0].toString(),
      members.voter.publicKey.toString()
    );
  });

  it("error: update policy with locked account index", async () => {
    // Create new autonomous smart account - account_utilization starts at 0
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

    // First create a valid policy with account index 0 (which is unlocked)
    const policyCreationPayload: smartAccount.generated.PolicyCreationPayload =
      {
        __kind: "InternalFundTransfer",
        fields: [
          {
            sourceAccountIndices: new Uint8Array([0]),
            destinationAccountIndices: new Uint8Array([0]),
            allowedMints: [web3.PublicKey.default],
          },
        ],
      };

    let transactionIndex = BigInt(1);

    const [policyPda] = smartAccount.getPolicyPda({
      settingsPda,
      policySeed,
      programId,
    });

    // Create, propose, approve, and execute the policy creation
    let signature = await rpc.createSettingsTransaction({
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
          expirationArgs: null,
        },
      ],
      programId,
    });
    await connection.confirmTransaction(signature);

    signature = await rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    signature = await rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    signature = await rpc.executeSettingsTransaction({
      connection,
      feePayer: members.almighty,
      settingsPda,
      transactionIndex,
      signer: members.almighty,
      rentPayer: members.almighty,
      policies: [policyPda],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Now try to update the policy to use locked account indices
    transactionIndex = BigInt(2);

    const policyUpdatePayload: smartAccount.generated.PolicyCreationPayload = {
      __kind: "InternalFundTransfer",
      fields: [
        {
          sourceAccountIndices: new Uint8Array([0, 5]), // Index 5 is locked
          destinationAccountIndices: new Uint8Array([0]),
          allowedMints: [web3.PublicKey.default],
        },
      ],
    };

    signature = await rpc.createSettingsTransaction({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer.publicKey,
      actions: [
        {
          __kind: "PolicyUpdate",
          policy: policyPda,
          policyUpdatePayload,
          signers: [
            {
              key: members.voter.publicKey,
              permissions: { mask: 7 },
            },
          ],
          threshold: 1,
          timeLock: 0,
          expirationArgs: null,
        },
      ],
      programId,
    });
    await connection.confirmTransaction(signature);

    signature = await rpc.createProposal({
      connection,
      feePayer: members.proposer,
      settingsPda,
      transactionIndex,
      creator: members.proposer,
      programId,
    });
    await connection.confirmTransaction(signature);

    signature = await rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda,
      transactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Execute should fail with AccountIndexLocked
    await assert.rejects(
      async () => {
        await smartAccount.rpc
          .executeSettingsTransaction({
            connection,
            feePayer: members.almighty,
            settingsPda,
            transactionIndex,
            signer: members.almighty,
            rentPayer: members.almighty,
            policies: [policyPda],
            programId,
          })
          .catch(smartAccount.errors.translateAndThrowAnchorError);
      },
      /AccountIndexLocked/
    );
  });
});
}
