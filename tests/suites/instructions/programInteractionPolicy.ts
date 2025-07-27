import * as smartAccount from "@sqds/smart-account";
import * as web3 from "@solana/web3.js";
import assert from "assert";
import {
  createAutonomousMultisig,
  createLocalhostConnection,
  createMintAndTransferTo,
  generateSmartAccountSigners,
  getTestProgramId,
  TestMembers,
} from "../../utils";
import { AccountMeta } from "@solana/web3.js";
import { getSmartAccountPda, generated, utils } from "@sqds/smart-account";
import {
  createAssociatedTokenAccount,
  createTransferInstruction,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  TOKEN_PROGRAM_ID,
  transfer,
} from "@solana/spl-token";

const { Settings, Proposal, Policy } = smartAccount.accounts;
const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / policy_settings_actions", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("Program Interaction Policy", async () => {
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

    let [sourceSmartAccountPda] = await getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    let [destinationSmartAccountPda] = await getSmartAccountPda({
      settingsPda,
      accountIndex: 1,
      programId,
    });

    let [mint, mintDecimals] = await createMintAndTransferTo(
      connection,
      members.voter,
      sourceSmartAccountPda,
      1_500_000_000
    );

    let sourceTokenAccount = await getAssociatedTokenAddressSync(
      mint,
      sourceSmartAccountPda,
      true
    );
    let destinationTokenAccount = getAssociatedTokenAddressSync(
      mint,
      destinationSmartAccountPda,
      true
    );

    await getOrCreateAssociatedTokenAccount(
      connection,
      members.voter,
      mint,
      destinationSmartAccountPda,
      true
    );

    // Create policy creation payload
    const policyCreationPayload: smartAccount.generated.PolicyCreationPayload =
      {
        __kind: "ProgramInteraction",
        fields: [
          {
            accountIndex: 0,
            instructionsConstraints: [
              {
                programId: TOKEN_PROGRAM_ID,
                dataConstraints: [
                  {
                    dataOffset: 0,
                    // Only allow TokenProgram.Transfer
                    dataValue: { __kind: "U8", fields: [3] },
                    // Only allow TokenProgram.Transfer
                    operator: generated.DataOperator.Equals,
                  },
                ],
                accountConstraints: [
                  {
                    // Destination of the transfer
                    accountIndex: 1,
                    accountKeys: [destinationTokenAccount],
                  },
                ],
              },
            ],
            spendingLimits: [
              {
                mint,
                timeConstraints: {
                  start: 0,
                  expiration: null,
                  // 10 Second spending limit
                  period: { __kind: "Custom", fields: [5] },
                },
                quantityConstraints: {
                  maxPerPeriod: 1_000_000_000,
                },
              },
            ],
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
      sendOptions: {
        skipPreflight: true,
      },
      programId,
    });
    await connection.confirmTransaction(signature);

    // Check the policy state
    const policyAccount = await Policy.fromAccountAddress(
      connection,
      policyPda
    );
    let policyState = policyAccount.policyState;
    let programInteractionPolicy = policyState
      .fields[0] as smartAccount.generated.ProgramInteractionPolicy;
    let spendingLimit = programInteractionPolicy.spendingLimits[0];
    assert.equal(
      spendingLimit.usage.remainingInPeriod.toString(),
      "1000000000"
    );
    let lastReset = spendingLimit.usage.lastReset.toString();

    assert.strictEqual(
      policyAccount.settings.toString(),
      settingsPda.toString()
    );
    assert.strictEqual(policyAccount.threshold, 1);
    assert.strictEqual(policyAccount.timeLock, 0);

    // Try transfer SOL via creating a transaction and proposal
    const policyTransactionIndex = BigInt(1);

    // Create SPL token transfer instruction
    const tokenTransferIxn = createTransferInstruction(
      sourceTokenAccount, // source token account
      destinationTokenAccount, // destination token account
      sourceSmartAccountPda, // authority
      500_000_000n // amount
    );

    // Create transaction message
    const message = new web3.TransactionMessage({
      payerKey: sourceSmartAccountPda,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [tokenTransferIxn],
    });

    let { transactionMessageBytes, compiledMessage } =
      utils.transactionMessageToMultisigTransactionMessageBytes({
        message,
        addressLookupTableAccounts: [],
        smartAccountPda: sourceSmartAccountPda,
      });

    const policyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "ProgramInteraction",
      fields: [
        {
          instructionConstraintIndices: new Uint8Array([0]),
          transactionPayload: {
            __kind: "AsyncTransaction",
            fields: [
              {
                accountIndex: 0,
                ephemeralSigners: 0,
                transactionMessage: transactionMessageBytes,
                memo: null,
              },
            ],
          },
        },
      ],
    };

    // Create a transaction
    signature = await smartAccount.rpc.createPolicyTransaction({
      connection,
      feePayer: members.voter,
      policy: policyPda,
      accountIndex: 0,
      transactionIndex: policyTransactionIndex,
      creator: members.voter.publicKey,
      policyPayload,
      sendOptions: {
        skipPreflight: true,
      },
      programId,
    });
    await connection.confirmTransaction(signature);

    // Create proposal for the transaction
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: members.voter,
      settingsPda: policyPda,
      transactionIndex: policyTransactionIndex,
      creator: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    // Approve the proposal (1/1 threshold)
    signature = await smartAccount.rpc.approveProposal({
      connection,
      feePayer: members.voter,
      settingsPda: policyPda,
      transactionIndex: policyTransactionIndex,
      signer: members.voter,
      programId,
    });
    await connection.confirmTransaction(signature);

    let remainingAccounts: AccountMeta[] = [];

    for (const [
      index,
      accountKey,
    ] of compiledMessage.staticAccountKeys.entries()) {
      if (accountKey.equals(sourceSmartAccountPda)) {
        remainingAccounts.push({
          pubkey: accountKey,
          isWritable: compiledMessage.isAccountWritable(index),
          isSigner: false,
        });
      } else {
        remainingAccounts.push({
          pubkey: accountKey,
          isWritable: compiledMessage.isAccountWritable(index),
          isSigner: false,
        });
      }
    }
    // Airdrop SOL to the source smart account
    let airdropSignature = await connection.requestAirdrop(
      sourceSmartAccountPda,
      2_000_000_000
    );
    await connection.confirmTransaction(airdropSignature);

    // Execute the transaction
    signature = await smartAccount.rpc.executePolicyTransaction({
      connection,
      feePayer: members.voter,
      policy: policyPda,
      transactionIndex: policyTransactionIndex,
      signer: members.voter.publicKey,
      anchorRemainingAccounts: remainingAccounts,
      sendOptions: {
        skipPreflight: true,
      },
      programId,
    });
    await connection.confirmTransaction(signature);

    // Check the balances & policy state
    let sourceBalance = await connection.getTokenAccountBalance(
      sourceTokenAccount
    );
    let destinationBalance = await connection.getTokenAccountBalance(
      destinationTokenAccount
    );
    assert.strictEqual(sourceBalance.value.amount, "1000000000");
    assert.strictEqual(destinationBalance.value.amount, "500000000");

    // Policy state
    let policyData = await Policy.fromAccountAddress(connection, policyPda, {
      commitment: "processed",
    });
    policyState = policyData.policyState;
    programInteractionPolicy = policyState
      .fields[0] as smartAccount.generated.ProgramInteractionPolicy;
    spendingLimit = programInteractionPolicy.spendingLimits[0];
    assert.equal(spendingLimit.usage.remainingInPeriod.toString(), "500000000");

    let modifiedTokenTransfer = tokenTransferIxn;
    modifiedTokenTransfer.keys[2].isWritable = true;

    let synchronousPayload = utils.instructionsToSynchronousTransactionDetails({
      vaultPda: sourceSmartAccountPda,
      members: [members.voter.publicKey],
      transaction_instructions: [tokenTransferIxn],
    });

    let syncPolicyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "ProgramInteraction",
      fields: [
        {
          instructionConstraintIndices: new Uint8Array([0]),
          transactionPayload: {
            __kind: "SyncTransaction",
            fields: [
              {
                accountIndex: 0,
                instructions: synchronousPayload.instructions,
              },
            ],
          },
        },
      ],
    };

    // Attempt to do the same with a synchronous instruction
    signature = await smartAccount.rpc.executePolicyPayloadSync({
      connection,
      feePayer: members.voter,
      policy: policyPda,
      accountIndex: 0,
      numSigners: 1,
      policyPayload: syncPolicyPayload,
      instruction_accounts: synchronousPayload.accounts,
      sendOptions: {
        skipPreflight: true,
      },
      signers: [members.voter],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Check the balances & policy state
    sourceBalance = await connection.getTokenAccountBalance(sourceTokenAccount);
    destinationBalance = await connection.getTokenAccountBalance(
      destinationTokenAccount
    );
    assert.strictEqual(sourceBalance.value.amount, "500000000");
    assert.strictEqual(destinationBalance.value.amount, "1000000000");

    // Check the policy state
    policyData = await Policy.fromAccountAddress(connection, policyPda, {
      commitment: "processed",
    });
    policyState = policyData.policyState;
    assert.strictEqual(policyState.__kind, "ProgramInteraction");
    programInteractionPolicy = policyState
      .fields[0] as smartAccount.generated.ProgramInteractionPolicy;
    spendingLimit = programInteractionPolicy.spendingLimits[0];
    assert.equal(spendingLimit.usage.remainingInPeriod.toString(), "0");
    assert.equal(spendingLimit.usage.lastReset.toString(), lastReset);

    // Try to transfer more than the policy allows
    await assert.rejects(
      smartAccount.rpc.executePolicyPayloadSync({
        connection,
        feePayer: members.voter,
        policy: policyPda,
        accountIndex: 0,
        numSigners: 1,
        policyPayload: syncPolicyPayload,
        instruction_accounts: synchronousPayload.accounts,
        signers: [members.voter],
        programId,
      }),
      (err: any) => {
        assert.ok(
          err
            .toString()
            .includes("ProgramInteractionInsufficientTokenAllowance")
        );
        return true;
      }
    );
    // Wait 6 seconds and retry to get the spending limit to reset
    await new Promise((resolve) => setTimeout(resolve, 5000));

    let signatureAfter = await smartAccount.rpc.executePolicyPayloadSync({
      connection,
      feePayer: members.voter,
      policy: policyPda,
      accountIndex: 0,
      numSigners: 1,
      policyPayload: syncPolicyPayload,
      instruction_accounts: synchronousPayload.accounts,
      signers: [members.voter],
      sendOptions: {
        skipPreflight: true,
      },
      programId,
    });
    await connection.confirmTransaction(signatureAfter);

    // Check the balances & policy state
    sourceBalance = await connection.getTokenAccountBalance(sourceTokenAccount);
    destinationBalance = await connection.getTokenAccountBalance(
      destinationTokenAccount
    );
    assert.strictEqual(sourceBalance.value.amount, "0");
    assert.strictEqual(destinationBalance.value.amount, "1500000000");

    // Policy state
    policyData = await Policy.fromAccountAddress(connection, policyPda, {
      commitment: "processed",
    });
    policyState = policyData.policyState;
    programInteractionPolicy = policyState
      .fields[0] as smartAccount.generated.ProgramInteractionPolicy;
    spendingLimit = programInteractionPolicy.spendingLimits[0];
    // Should have reset
    assert.equal(spendingLimit.usage.remainingInPeriod.toString(), "500000000");
  });
});
