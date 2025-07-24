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
import { getSmartAccountPda } from "@sqds/smart-account";
import {
  createAssociatedTokenAccount,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
const { Settings, Proposal, Policy } = smartAccount.accounts;
const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / policy_settings_actions", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("create policy: InternalFundTransfer + SOL Transfer", async () => {
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
            sourceAccountIndices: new Uint8Array([0]), // Allow transfers from account indices 0 and 1
            destinationAccountIndices: new Uint8Array([1, 3]), // Allow transfers to account indices 2 and 3
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
      __kind: "InternalFundTransfer",
      fields: [
        {
          sourceIndex: 0,
          destinationIndex: 1,
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
    remainingAccounts.push({
      pubkey: sourceSmartAccountPda,
      isWritable: true,
      isSigner: false,
    });
    remainingAccounts.push({
      pubkey: destinationSmartAccountPda,
      isWritable: true,
      isSigner: false,
    });
    remainingAccounts.push({
      pubkey: web3.SystemProgram.programId,
      isWritable: false,
      isSigner: false,
    });
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

    // Check the balances
    let sourceBalance = await connection.getBalance(sourceSmartAccountPda);
    let destinationBalance = await connection.getBalance(
      destinationSmartAccountPda
    );
    assert.strictEqual(sourceBalance, 1_000_000_000);
    assert.strictEqual(destinationBalance, 1_000_000_000);

    // Attempt to do the same with a synchronous instruction
    signature = await smartAccount.rpc.executePolicyPayloadSync({
      connection,
      feePayer: members.voter,
      policy: policyPda,
      accountIndex: 0,
      numSigners: 1,
      policyPayload: policyPayload,
      instruction_accounts: remainingAccounts,
      sendOptions: {
        skipPreflight: true,
      },
      signers: [members.voter],
      programId,
    });
    await connection.confirmTransaction(signature);

    // Check the balances
    sourceBalance = await connection.getBalance(sourceSmartAccountPda);
    destinationBalance = await connection.getBalance(
      destinationSmartAccountPda
    );
    assert.strictEqual(sourceBalance, 0);
    assert.strictEqual(destinationBalance, 2_000_000_000);

    let invalidPayload: smartAccount.generated.PolicyPayload = {
      __kind: "InternalFundTransfer",
      fields: [
        {
          // Invalid source index
          sourceIndex: 1,
          destinationIndex: 1,
          mint: web3.PublicKey.default,
          decimals: 9,
          amount: 1_000_000_000,
        },
      ],
    };
    assert.rejects(
      smartAccount.rpc.executePolicyPayloadSync({
        connection,
        feePayer: members.voter,
        policy: policyPda,
        accountIndex: 0,
        numSigners: 1,
        policyPayload: invalidPayload,
        instruction_accounts: remainingAccounts,
        signers: [members.voter],
        programId,
      })
    );
  });

  it("InternalFundTransfer + SPL Token Transfer", async () => {
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

    // Create a mint and transfer tokens to the source smart account
    let [sourceSmartAccountPda] = getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });
    let [destinationSmartAccountPda] = getSmartAccountPda({
      settingsPda,
      accountIndex: 1,
      programId,
    });
    const [mint, mintDecimals] = await createMintAndTransferTo(
      connection,
      members.voter,
      sourceSmartAccountPda,
      1_000_000_000
    );

    // Create policy creation payload
    const policyCreationPayload: smartAccount.generated.PolicyCreationPayload =
      {
        __kind: "InternalFundTransfer",
        fields: [
          {
            sourceAccountIndices: new Uint8Array([0]), // Allow transfers from account indices 0 and 1
            destinationAccountIndices: new Uint8Array([1, 3]), // Allow transfers to account indices 2 and 3
            allowedMints: [mint], // Allow native SOL transfers
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

    const policyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "InternalFundTransfer",
      fields: [
        {
          sourceIndex: 0,
          destinationIndex: 1,
          mint: mint,
          decimals: mintDecimals,
          amount: 500_000_000,
        },
      ],
    };
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

    // Attempt to do the same with a synchronous instruction
    signature = await smartAccount.rpc.executePolicyPayloadSync({
      connection,
      feePayer: members.voter,
      policy: policyPda,
      accountIndex: 0,
      numSigners: 1,
      policyPayload: policyPayload,
      instruction_accounts: remainingAccounts,
      sendOptions: {
        skipPreflight: true,
      },
      signers: [members.voter],
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
    assert.strictEqual(sourceBalance.value.amount, "500000000");
    assert.strictEqual(destinationBalance.value.amount, "500000000");
  });
});
