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
  createApproveInstruction,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

const { Policy } = smartAccount.accounts;
const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Audit / InternalFundTransfer destination delegate check", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("InternalFundTransfer fails when destination token account has delegate", async () => {
    // Source at index 1, destination at index 0.
    // Index 0 is always unlocked so we can set the delegate via V1 sync.
    const settingsPda = (
      await createAutonomousMultisig({
        connection,
        members,
        threshold: 1,
        timeLock: 0,
        programId,
      })
    )[0];

    // Increment account_utilization to unlock index 1
    const incrementIx =
      smartAccount.generated.createIncrementAccountIndexInstruction(
        {
          settings: settingsPda,
          signer: members.almighty.publicKey,
          program: programId,
        },
        programId
      );
    const incrementMsg = new web3.TransactionMessage({
      payerKey: members.almighty.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [incrementIx],
    }).compileToV0Message();
    const incrementTx = new web3.VersionedTransaction(incrementMsg);
    incrementTx.sign([members.almighty]);
    await connection.confirmTransaction(
      await connection.sendRawTransaction(incrementTx.serialize())
    );

    const policySeed = 1;

    const [destinationSmartAccountPda] = getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });
    const [sourceSmartAccountPda] = getSmartAccountPda({
      settingsPda,
      accountIndex: 1,
      programId,
    });

    // Create mint and fund source vault (index 1)
    const [mint, mintDecimals] = await createMintAndTransferTo(
      connection,
      members.voter,
      sourceSmartAccountPda,
      1_000_000_000
    );

    // Create destination ATA (index 0)
    const destinationTokenAccount = getAssociatedTokenAddressSync(
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

    // --- Set delegate on destination token account via V1 sync transaction ---
    const delegateKey = web3.Keypair.generate().publicKey;
    const approveIx = createApproveInstruction(
      destinationTokenAccount,
      delegateKey,
      destinationSmartAccountPda,
      1
    );
    // Mark owner/authority writable so compiled-keys has at least one writable signer
    approveIx.keys[2].isWritable = true;

    const { instructions: syncInstructions, accounts: syncAccounts } =
      smartAccount.utils.instructionsToSynchronousTransactionDetails({
        vaultPda: destinationSmartAccountPda,
        members: [members.almighty.publicKey],
        transaction_instructions: [approveIx],
      });

    const syncIx = smartAccount.instructions.executeTransactionSync({
      settingsPda,
      numSigners: 1,
      accountIndex: 0,
      instructions: syncInstructions,
      instruction_accounts: syncAccounts,
      programId,
    });

    const syncMessage = new web3.TransactionMessage({
      payerKey: members.almighty.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [syncIx],
    }).compileToV0Message();

    const syncTx = new web3.VersionedTransaction(syncMessage);
    syncTx.sign([members.almighty]);
    let signature = await connection.sendRawTransaction(syncTx.serialize());
    await connection.confirmTransaction(signature);

    // --- Create InternalFundTransferPolicy (source: 1 → destination: 0) ---
    const policyCreationPayload: smartAccount.generated.PolicyCreationPayload = {
      __kind: "InternalFundTransfer",
      fields: [
        {
          sourceAccountIndices: new Uint8Array([1]),
          destinationAccountIndices: new Uint8Array([0]),
          allowedMints: [mint],
        },
      ],
    };

    const transactionIndex = BigInt(1);
    const [policyPda] = smartAccount.getPolicyPda({
      settingsPda,
      policySeed,
      programId,
    });

    signature = await smartAccount.rpc.createSettingsTransaction({
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

    // --- Attempt InternalFundTransfer: source (1) → destination (0) ---
    const sourceTokenAccount = getAssociatedTokenAddressSync(
      mint,
      sourceSmartAccountPda,
      true
    );

    const policyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "InternalFundTransfer",
      fields: [
        {
          sourceIndex: 1,
          destinationIndex: 0,
          mint: mint,
          decimals: mintDecimals,
          amount: 500_000_000,
        },
      ],
    };

    let remainingAccounts: AccountMeta[] = [
      {
        pubkey: members.voter.publicKey,
        isWritable: false,
        isSigner: true,
      },
      {
        pubkey: sourceSmartAccountPda,
        isWritable: false,
        isSigner: false,
      },
      {
        pubkey: sourceTokenAccount,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: destinationTokenAccount,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: mint,
        isWritable: false,
        isSigner: false,
      },
      {
        pubkey: TOKEN_PROGRAM_ID,
        isWritable: false,
        isSigner: false,
      },
    ];

    // Should fail because destination token account has a delegate.
    await assert.rejects(
      () =>
        smartAccount.rpc.executePolicyPayloadSync({
          connection,
          feePayer: members.voter,
          policy: policyPda,
          accountIndex: 0,
          numSigners: 1,
          policyPayload: policyPayload,
          instruction_accounts: remainingAccounts,
          signers: [members.voter],
          programId,
        }),
      /InternalFundTransferPolicyDestinationHasDelegate/
    );
  });
});
