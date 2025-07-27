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
import { AccountMeta } from "@solana/web3.js";
const { Settings, Proposal, Policy } = smartAccount.accounts;
const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Flows / Policy Update", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("create policy: InternalFundTransfer", async () => {
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
          expiration: null,
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
    signature = await smartAccount.rpc.createPolicyTransaction({
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

    let updateSignature = await smartAccount.rpc.executeSettingsTransactionSync(
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
            expiration: null,
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
});
