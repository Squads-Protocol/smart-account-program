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

describe("Instructions / spending_limit_policy", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("Spending Limit Policy", async () => {
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
      1_000_000_000
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
        __kind: "SpendingLimit",
        fields: [
          {
            mint,
            sourceAccountIndex: 0,
            destinations: [destinationSmartAccountPda],
            timeConstraints: {
              start: 0,
              expiration: null,
              period: { __kind: "Day" },
              accumulateUnused: false,
            },
            quantityConstraints: {
              maxPerPeriod: 750_000_000,
              maxPerUse: 250_000_000,
              enforceExactQuantity: true,
            },
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

    const policyAccount = await Policy.fromAccountAddress(
      connection,
      policyPda
    );
    assert.strictEqual(
      policyAccount.settings.toString(),
      settingsPda.toString()
    );
    assert.strictEqual(policyAccount.threshold, 1);
    assert.strictEqual(policyAccount.timeLock, 0);

    // Try transfer SOL via creating a transaction and proposal
    const policyTransactionIndex = BigInt(1);

    const policyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "SpendingLimit",
      fields: [
        {
          amount: 250_000_000,
          destination: destinationSmartAccountPda,
          decimals: mintDecimals,
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
    remainingAccounts.push({
      pubkey: sourceSmartAccountPda,
      isWritable: false,
      isSigner: false,
    });
    remainingAccounts.push({
      pubkey: sourceTokenAccount,
      isWritable: true,
      isSigner: false,
    });
    remainingAccounts.push({
      pubkey: destinationTokenAccount,
      isWritable: true,
      isSigner: false,
    });
    remainingAccounts.push({
      pubkey: mint,
      isWritable: false,
      isSigner: false,
    });
    remainingAccounts.push({
      pubkey: TOKEN_PROGRAM_ID,
      isWritable: false,
      isSigner: false,
    });

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

    // Check the balances
    let sourceBalance = await connection.getTokenAccountBalance(
      sourceTokenAccount
    );
    let destinationBalance = await connection.getTokenAccountBalance(
      destinationTokenAccount
    );
    assert.strictEqual(sourceBalance.value.amount, "750000000");
    assert.strictEqual(destinationBalance.value.amount, "250000000");

    let syncPolicyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "SpendingLimit",
      fields: [
        {
          amount: 250_000_000,
          destination: destinationSmartAccountPda,
          decimals: mintDecimals,
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
      instruction_accounts: remainingAccounts,
      sendOptions: {
        skipPreflight: true,
      },
      signers: [members.voter],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Check the balances
    sourceBalance = await connection.getTokenAccountBalance(sourceTokenAccount);
    destinationBalance = await connection.getTokenAccountBalance(
      destinationTokenAccount
    );
    assert.strictEqual(sourceBalance.value.amount, "500000000");
    assert.strictEqual(destinationBalance.value.amount, "500000000");

    let invalidPayload: smartAccount.generated.PolicyPayload = {
      __kind: "SpendingLimit",
      fields: [
        {
          amount: 250_000_001,
          destination: destinationSmartAccountPda,
          decimals: mintDecimals,
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
      policyPayload: invalidPayload,
      instruction_accounts: remainingAccounts,
      sendOptions: {
        skipPreflight: true,
      },
      //signers: [members.voter],
      programId,
    });
    await connection.confirmTransaction(signature);
    console.log(signature);
  });
});
