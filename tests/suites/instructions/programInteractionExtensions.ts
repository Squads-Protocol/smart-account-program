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
import { getSmartAccountPda, generated, utils } from "@sqds/smart-account";
import {
  createEnableRequiredMemoTransfersInstruction,
  createInitializeAccount3Instruction,
  createInitializeMint2Instruction,
  createMintToInstruction,
  ExtensionType,
  getAccountLen,
  MINT_SIZE,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token";

const { Policy } = smartAccount.accounts;
const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Audit / ProgramInteraction T22 extension modification detection", () => {
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  it("CPI that modifies T22 extension data fails with ProgramInteractionIllegalTokenAccountModification", async () => {
    const settingsPda = (
      await createAutonomousMultisig({
        connection,
        members,
        threshold: 1,
        timeLock: 0,
        programId,
      })
    )[0];

    const [sourceSmartAccountPda] = getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });

    // --- Create T22 mint (external authority, not vault PDA) ---
    const mintKeypair = web3.Keypair.generate();
    const mintRent = await connection.getMinimumBalanceForRentExemption(
      MINT_SIZE
    );

    const mintMessage = new web3.TransactionMessage({
      payerKey: members.voter.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [
        web3.SystemProgram.createAccount({
          fromPubkey: members.voter.publicKey,
          newAccountPubkey: mintKeypair.publicKey,
          space: MINT_SIZE,
          lamports: mintRent,
          programId: TOKEN_2022_PROGRAM_ID,
        }),
        createInitializeMint2Instruction(
          mintKeypair.publicKey,
          9,
          members.voter.publicKey,
          null,
          TOKEN_2022_PROGRAM_ID
        ),
      ],
    }).compileToV0Message();

    const mintTx = new web3.VersionedTransaction(mintMessage);
    mintTx.sign([members.voter, mintKeypair]);
    let signature = await connection.sendTransaction(mintTx);
    await connection.confirmTransaction(signature);

    // --- Create T22 token account with MemoTransfer extension space ---
    const tokenAccountKeypair = web3.Keypair.generate();
    const accountSpace = getAccountLen([ExtensionType.MemoTransfer]);
    const accountRent =
      await connection.getMinimumBalanceForRentExemption(accountSpace);

    const createAccountMessage = new web3.TransactionMessage({
      payerKey: members.voter.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [
        web3.SystemProgram.createAccount({
          fromPubkey: members.voter.publicKey,
          newAccountPubkey: tokenAccountKeypair.publicKey,
          space: accountSpace,
          lamports: accountRent,
          programId: TOKEN_2022_PROGRAM_ID,
        }),
        createInitializeAccount3Instruction(
          tokenAccountKeypair.publicKey,
          mintKeypair.publicKey,
          sourceSmartAccountPda,
          TOKEN_2022_PROGRAM_ID
        ),
      ],
    }).compileToV0Message();

    const createAccountTx = new web3.VersionedTransaction(
      createAccountMessage
    );
    createAccountTx.sign([members.voter, tokenAccountKeypair]);
    signature = await connection.sendTransaction(createAccountTx);
    await connection.confirmTransaction(signature);

    // --- Mint tokens to the T22 token account ---
    const mintToMessage = new web3.TransactionMessage({
      payerKey: members.voter.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [
        createMintToInstruction(
          mintKeypair.publicKey,
          tokenAccountKeypair.publicKey,
          members.voter.publicKey,
          1_000_000_000,
          [],
          TOKEN_2022_PROGRAM_ID
        ),
      ],
    }).compileToV0Message();

    const mintToTx = new web3.VersionedTransaction(mintToMessage);
    mintToTx.sign([members.voter]);
    signature = await connection.sendTransaction(mintToTx);
    await connection.confirmTransaction(signature);

    // --- Create ProgramInteractionPolicy allowing TOKEN_2022_PROGRAM_ID ---
    const policySeed = 1;
    const transactionIndex = BigInt(1);

    const policyCreationPayload: smartAccount.generated.PolicyCreationPayload = {
      __kind: "LegacyProgramInteraction",
      fields: [
        {
          accountIndex: 0,
          preHook: null,
          postHook: null,
          instructionsConstraints: [
            {
              programId: TOKEN_2022_PROGRAM_ID,
              dataConstraints: [],
              accountConstraints: [],
            },
          ],
          spendingLimits: [
            {
              mint: mintKeypair.publicKey,
              timeConstraints: {
                start: 0,
                expiration: null,
                period: { __kind: "OneTime" as const },
              },
              quantityConstraints: {
                maxPerPeriod: 1_000_000_000_000,
              },
            },
          ],
        },
      ],
    };

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

    // --- Build CPI to enable MemoTransfer (modifies extension data) ---
    const enableMemoIx = createEnableRequiredMemoTransfersInstruction(
      tokenAccountKeypair.publicKey,
      sourceSmartAccountPda,
      [],
      TOKEN_2022_PROGRAM_ID
    );
    // Mark authority writable for sync utility compatibility
    enableMemoIx.keys[1].isWritable = true;

    const syncPayload =
      utils.instructionsToSynchronousTransactionDetailsV2WithHooks({
        vaultPda: sourceSmartAccountPda,
        members: [members.voter.publicKey],
        preHookAccounts: [],
        postHookAccounts: [],
        transaction_instructions: [enableMemoIx],
      });

    const syncPolicyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "ProgramInteraction",
      fields: [
        {
          instructionConstraintIndices: new Uint8Array([0]),
          transactionPayload: {
            __kind: "SyncTransaction",
            fields: [
              {
                accountIndex: 0,
                instructions: syncPayload.instructions,
              },
            ],
          },
        },
      ],
    };

    // Should fail: extension hash changed after CPI
    await assert.rejects(
      () =>
        smartAccount.rpc.executePolicyPayloadSync({
          connection,
          feePayer: members.voter,
          policy: policyPda,
          accountIndex: 0,
          numSigners: 1,
          policyPayload: syncPolicyPayload,
          instruction_accounts: syncPayload.accounts,
          signers: [members.voter],
          programId,
        }),
      /ProgramInteractionIllegalTokenAccountModification/
    );
  });
});
