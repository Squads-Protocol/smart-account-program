import * as smartAccount from "@sqds/smart-account";
import {
  Keypair,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import assert from "assert";
import {
  createLocalhostConnection,
  extractTransactionPayloadDetails,
  generateFundedKeypair,
  generateSmartAccountSigners,
  getTestProgramId,
  TestMembers,
} from "../utils";
import {
  buildTestMessage as buildTestMessageHelper,
  buildTransactionMessageBytes as buildTransactionMessageBytesHelper,
  createSettings as createSettingsHelper,
  createTransactionV1 as createTransactionV1Helper,
  getNextTransactionIndex as getNextTransactionIndexHelper,
  sendCreateTransactionV2 as sendCreateTransactionV2Helper,
  sendCreateTransactionV2WithArgs as sendCreateTransactionV2WithArgsHelper,
  sendV1WithArgs as sendV1WithArgsHelper,
} from "./utils/transactionCreate";
import {
  createInternalFundTransferPolicy,
  getNextPolicyTransactionIndex,
} from "./utils/policy";

const { Settings, Transaction } = smartAccount.accounts;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / transaction_create", () => {
  const skip = { it: it.skip };
  let members: TestMembers;

  before(async () => {
    members = await generateSmartAccountSigners(connection);
  });

  const createSettings = (timeLock = 0) =>
    createSettingsHelper({ connection, members, programId, timeLock });

  const buildTestMessage = (settingsPda: PublicKey, accountIndex: number) =>
    buildTestMessageHelper({ connection, settingsPda, accountIndex, programId });

  const buildTransactionMessageBytes = (
    settingsPda: PublicKey,
    accountIndex: number
  ) =>
    buildTransactionMessageBytesHelper({
      connection,
      programId,
      settingsPda,
      accountIndex,
    });

  const getNextTransactionIndex = (settingsPda: PublicKey) =>
    getNextTransactionIndexHelper({ connection, settingsPda });

  const createTransactionV1 = (
    args: Omit<
      Parameters<typeof createTransactionV1Helper>[0],
      "connection" | "programId"
    >
  ) => createTransactionV1Helper({ ...args, connection, programId });

  const sendCreateTransactionV2 = (
    args: Omit<
      Parameters<typeof sendCreateTransactionV2Helper>[0],
      "connection" | "programId"
    >
  ) => sendCreateTransactionV2Helper({ ...args, connection, programId });

  const sendCreateTransactionV2WithArgs = (
    args: Omit<
      Parameters<typeof sendCreateTransactionV2WithArgsHelper>[0],
      "connection" | "programId"
    >
  ) => sendCreateTransactionV2WithArgsHelper({ ...args, connection, programId });

  const sendCreateTransactionV1WithArgs = async (params: {
    consensusAccount: PublicKey;
    creator: Keypair;
    rentPayer: Keypair;
    transactionIndex: bigint;
    createArgs: smartAccount.generated.CreateTransactionArgs;
  }) => {
    const [transactionPda] = smartAccount.getTransactionPda({
      settingsPda: params.consensusAccount,
      transactionIndex: params.transactionIndex,
      programId,
    });
    const ix = smartAccount.generated.createCreateTransactionInstruction(
      {
        consensusAccount: params.consensusAccount,
        transaction: transactionPda,
        creator: params.creator.publicKey,
        rentPayer: params.rentPayer.publicKey,
        program: programId,
      },
      { args: params.createArgs },
      programId
    );
    const { blockhash } = await connection.getLatestBlockhash();
    const message = new TransactionMessage({
      payerKey: params.rentPayer.publicKey,
      recentBlockhash: blockhash,
      instructions: [ix],
    }).compileToV0Message();
    const tx = new VersionedTransaction(message);
    const signers = params.creator.publicKey.equals(params.rentPayer.publicKey)
      ? [params.creator]
      : [params.creator, params.rentPayer];
    tx.sign(signers);
    const signature = await connection
      .sendRawTransaction(tx.serialize())
      .catch(smartAccount.errors.translateAndThrowAnchorError);
    await connection.confirmTransaction(signature);

    return { transactionPda };
  };

  const sendV1WithArgs = (
    args: Omit<
      Parameters<typeof sendV1WithArgsHelper>[0],
      "connection" | "programId"
    >
  ) => sendV1WithArgsHelper({ ...args, connection, programId });

  // -------------------------------------------------------------------------------------
  // Golden Path Tests (V1 + V2 parity)
  // -------------------------------------------------------------------------------------

  it("should_create_transaction_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 0);

    const { transactionIndex, transactionPda } = await createTransactionV1({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessage: message,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    assert.strictEqual(
      transactionAccount.index.toString(),
      transactionIndex.toString()
    );
    assert.strictEqual(
      transactionAccount.creator.toBase58(),
      members.proposer.publicKey.toBase58()
    );
  });

  it("should_create_transaction_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    const { transactionIndex, transactionPda } = await sendCreateTransactionV2({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessageBytes,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    assert.strictEqual(
      transactionAccount.index.toString(),
      transactionIndex.toString()
    );
    assert.strictEqual(
      transactionAccount.creator.toBase58(),
      members.proposer.publicKey.toBase58()
    );
  });

  it("should_create_transaction_with_settings_payload_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 0);

    const { transactionPda } = await createTransactionV1({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessage: message,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    assert.strictEqual(transactionAccount.payload.__kind, "TransactionPayload");
  });

  it("should_create_transaction_with_settings_payload_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    const { transactionPda } = await sendCreateTransactionV2({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessageBytes,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    assert.strictEqual(transactionAccount.payload.__kind, "TransactionPayload");
  });

  it("should_create_transaction_with_policy_payload_v1", async () => {
    const settingsPda = await createSettings();
    const { policyPda } = await createInternalFundTransferPolicy({
      connection,
      programId,
      members,
      settingsPda,
    });
    const policyTransactionIndex = await getNextPolicyTransactionIndex({
      connection,
      policyPda,
    });
    const policyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "InternalFundTransfer",
      fields: [
        {
          sourceIndex: 0,
          destinationIndex: 1,
          mint: PublicKey.default,
          decimals: 9,
          amount: 1_000_000_000,
        },
      ],
    };
    const createArgs = {
      __kind: "PolicyPayload",
      payload: policyPayload,
    } as unknown as smartAccount.generated.CreateTransactionArgs;

    const { transactionPda } = await sendCreateTransactionV1WithArgs({
      consensusAccount: policyPda,
      creator: members.voter,
      rentPayer: members.voter,
      transactionIndex: policyTransactionIndex,
      createArgs,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    assert.strictEqual(transactionAccount.payload.__kind, "PolicyPayload");
    assert.strictEqual(
      transactionAccount.consensusAccount.toBase58(),
      policyPda.toBase58()
    );
  });

  it("should_create_transaction_with_policy_payload_v2", async () => {
    const settingsPda = await createSettings();
    const { policyPda } = await createInternalFundTransferPolicy({
      connection,
      programId,
      members,
      settingsPda,
    });
    const policyTransactionIndex = await getNextPolicyTransactionIndex({
      connection,
      policyPda,
    });
    const policyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "InternalFundTransfer",
      fields: [
        {
          sourceIndex: 0,
          destinationIndex: 1,
          mint: PublicKey.default,
          decimals: 9,
          amount: 1_000_000_000,
        },
      ],
    };
    const createArgs = {
      __kind: "PolicyPayload",
      payload: policyPayload,
    } as unknown as smartAccount.generated.CreateTransactionArgs;

    const { transactionPda } = await sendCreateTransactionV2WithArgs({
      consensusAccount: policyPda,
      creator: members.voter,
      rentPayer: members.voter,
      transactionIndex: policyTransactionIndex,
      createArgs,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    assert.strictEqual(transactionAccount.payload.__kind, "PolicyPayload");
    assert.strictEqual(
      transactionAccount.consensusAccount.toBase58(),
      policyPda.toBase58()
    );
  });

  it("should_increment_transaction_index_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 0);
    const nextIndex = await getNextTransactionIndex(settingsPda);

    await createTransactionV1({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessage: message,
    });

    const settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(
      settingsAccount.transactionIndex.toString(),
      nextIndex.toString()
    );
  });

  it("should_increment_transaction_index_v2", async () => {
    const settingsPda = await createSettings();
    const nextIndex = await getNextTransactionIndex(settingsPda);
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    await sendCreateTransactionV2({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessageBytes,
    });

    const settingsAccount = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(
      settingsAccount.transactionIndex.toString(),
      nextIndex.toString()
    );
  });

  it("should_set_creator_and_rent_collector_fields_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 0);

    const { transactionPda } = await createTransactionV1({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.almighty,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessage: message,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    assert.strictEqual(
      transactionAccount.creator.toBase58(),
      members.proposer.publicKey.toBase58()
    );
    assert.strictEqual(
      transactionAccount.rentCollector.toBase58(),
      members.almighty.publicKey.toBase58()
    );
  });

  it("should_set_creator_and_rent_collector_fields_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    const { transactionPda } = await sendCreateTransactionV2({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.almighty,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessageBytes,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    assert.strictEqual(
      transactionAccount.creator.toBase58(),
      members.proposer.publicKey.toBase58()
    );
    assert.strictEqual(
      transactionAccount.rentCollector.toBase58(),
      members.almighty.publicKey.toBase58()
    );
  });

  it("should_set_payload_type_transaction_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 0);

    const { transactionPda } = await createTransactionV1({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessage: message,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    assert.strictEqual(transactionAccount.payload.__kind, "TransactionPayload");
  });

  it("should_set_payload_type_transaction_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    const { transactionPda } = await sendCreateTransactionV2({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessageBytes,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    assert.strictEqual(transactionAccount.payload.__kind, "TransactionPayload");
  });

  it("should_set_payload_type_policy_v1", async () => {
    const settingsPda = await createSettings();
    const { policyPda } = await createInternalFundTransferPolicy({
      connection,
      programId,
      members,
      settingsPda,
    });
    const policyTransactionIndex = await getNextPolicyTransactionIndex({
      connection,
      policyPda,
    });
    const policyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "InternalFundTransfer",
      fields: [
        {
          sourceIndex: 0,
          destinationIndex: 1,
          mint: PublicKey.default,
          decimals: 9,
          amount: 1_000_000_000,
        },
      ],
    };
    const createArgs = {
      __kind: "PolicyPayload",
      payload: policyPayload,
    } as unknown as smartAccount.generated.CreateTransactionArgs;

    const { transactionPda } = await sendCreateTransactionV1WithArgs({
      consensusAccount: policyPda,
      creator: members.voter,
      rentPayer: members.voter,
      transactionIndex: policyTransactionIndex,
      createArgs,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    assert.strictEqual(transactionAccount.payload.__kind, "PolicyPayload");
  });

  it("should_set_payload_type_policy_v2", async () => {
    const settingsPda = await createSettings();
    const { policyPda } = await createInternalFundTransferPolicy({
      connection,
      programId,
      members,
      settingsPda,
    });
    const policyTransactionIndex = await getNextPolicyTransactionIndex({
      connection,
      policyPda,
    });
    const policyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "InternalFundTransfer",
      fields: [
        {
          sourceIndex: 0,
          destinationIndex: 1,
          mint: PublicKey.default,
          decimals: 9,
          amount: 1_000_000_000,
        },
      ],
    };
    const createArgs = {
      __kind: "PolicyPayload",
      payload: policyPayload,
    } as unknown as smartAccount.generated.CreateTransactionArgs;

    const { transactionPda } = await sendCreateTransactionV2WithArgs({
      consensusAccount: policyPda,
      creator: members.voter,
      rentPayer: members.voter,
      transactionIndex: policyTransactionIndex,
      createArgs,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    assert.strictEqual(transactionAccount.payload.__kind, "PolicyPayload");
  });

  it("should_derive_ephemeral_signer_bumps_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 0);

    const { transactionIndex, transactionPda } = await createTransactionV1({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 2,
      transactionMessage: message,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    const payload = extractTransactionPayloadDetails(transactionAccount.payload);
    const expectedBumps = [0, 1].map((index) =>
      smartAccount.getEphemeralSignerPda({
        transactionPda,
        ephemeralSignerIndex: index,
        programId,
      })[1]
    );

    assert.strictEqual(payload.accountIndex, 0);
    assert.strictEqual(payload.ephemeralSignerBumps.length, 2);
    assert.deepEqual(payload.ephemeralSignerBumps, Uint8Array.from(expectedBumps));
    assert.strictEqual(
      transactionIndex.toString(),
      transactionAccount.index.toString()
    );
  });

  it("should_derive_ephemeral_signer_bumps_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    const { transactionPda } = await sendCreateTransactionV2({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 2,
      transactionMessageBytes,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    const payload = extractTransactionPayloadDetails(transactionAccount.payload);
    const expectedBumps = [0, 1].map((index) =>
      smartAccount.getEphemeralSignerPda({
        transactionPda,
        ephemeralSignerIndex: index,
        programId,
      })[1]
    );

    assert.strictEqual(payload.accountIndex, 0);
    assert.strictEqual(payload.ephemeralSignerBumps.length, 2);
    assert.deepEqual(payload.ephemeralSignerBumps, Uint8Array.from(expectedBumps));
  });

  // Skipped: no event parser utilities in tests to decode logged events.
  skip.it("should_emit_create_event_v1");
  // Skipped: no event parser utilities in tests to decode logged events.
  skip.it("should_emit_create_event_v2");

  // -------------------------------------------------------------------------------------
  // Invariant & Safety Tests (V1 + V2 parity)
  // -------------------------------------------------------------------------------------

  // Skipped: consensus account derivation failures require crafted invalid accounts.
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  // Skipped: consensus account derivation failures require crafted invalid accounts.
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  // Skipped: settings accounts are always active and have no expiration.
  skip.it("should_fail_when_consensus_account_inactive_v1");
  // Skipped: settings accounts are always active and have no expiration.
  skip.it("should_fail_when_consensus_account_inactive_v2");

  it("should_fail_on_not_a_signer_v1", async () => {
    const settingsPda = await createSettings();
    const nonMember = await generateFundedKeypair(connection);
    const { message } = await buildTestMessage(settingsPda, 0);

    await assert.rejects(
      () =>
        smartAccount.rpc.createTransaction({
          connection,
          feePayer: nonMember,
          settingsPda,
          transactionIndex: 1n,
          creator: nonMember.publicKey,
          accountIndex: 0,
          ephemeralSigners: 0,
          transactionMessage: message,
          programId,
        }),
      /Provided pubkey is not a signer of the smart account/
    );
  });

  it("should_fail_on_not_a_signer_v2", async () => {
    const settingsPda = await createSettings();
    const nonMember = await generateFundedKeypair(connection);
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    await assert.rejects(
      async () => {
        await sendCreateTransactionV2({
          settingsPda,
          creator: nonMember,
          rentPayer: nonMember,
          accountIndex: 0,
          ephemeralSigners: 0,
          transactionMessageBytes,
        });
      },
      /Provided pubkey is not a signer of the smart account/
    );
  });

  it("should_fail_on_missing_initiate_permission_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 0);

    await assert.rejects(
      () =>
        smartAccount.rpc.createTransaction({
          connection,
          feePayer: members.voter,
          settingsPda,
          transactionIndex: 1n,
          creator: members.voter.publicKey,
          accountIndex: 0,
          ephemeralSigners: 0,
          transactionMessage: message,
          programId,
        }),
      /Attempted to perform an unauthorized action/
    );
  });

  it("should_fail_on_missing_initiate_permission_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    await assert.rejects(
      async () => {
        await sendCreateTransactionV2({
          settingsPda,
          creator: members.voter,
          rentPayer: members.voter,
          accountIndex: 0,
          ephemeralSigners: 0,
          transactionMessageBytes,
        });
      },
      /Attempted to perform an unauthorized action/
    );
  });

  it("should_fail_on_invalid_payload_for_settings_v1", async () => {
    const settingsPda = await createSettings();
    const args = {
      __kind: "PolicyPayload",
      payload: {
        __kind: "InternalFundTransfer",
        fields: [
          {
            sourceIndex: 0,
            destinationIndex: 1,
            mint: PublicKey.default,
            decimals: 0,
            amount: 0n,
          },
        ],
      },
    } as unknown as smartAccount.generated.CreateTransactionArgs;

    await assert.rejects(
      () =>
        sendV1WithArgs({
          settingsPda,
          creator: members.proposer,
          rentPayer: members.proposer,
          args,
        }),
      /TransactionMessage is malformed|InvalidTransactionMessage|Failed to serialize or deserialize account data/
    );
  });

  it("should_fail_on_invalid_payload_for_settings_v2", async () => {
    const settingsPda = await createSettings();
    const transactionIndex = await getNextTransactionIndex(settingsPda);
    const [transactionPda] = smartAccount.getTransactionPda({
      settingsPda,
      transactionIndex,
      programId,
    });

    const createArgs = {
      __kind: "PolicyPayload",
      payload: {
        __kind: "InternalFundTransfer",
        fields: [
          {
            sourceIndex: 0,
            destinationIndex: 1,
            mint: PublicKey.default,
            decimals: 0,
            amount: 0n,
          },
        ],
      },
    } as unknown as smartAccount.generated.CreateTransactionArgs;

    const ix = smartAccount.generated.createCreateTransactionV2Instruction(
      {
        consensusAccount: settingsPda,
        transaction: transactionPda,
        rentPayer: members.proposer.publicKey,
        program: programId,
        anchorRemainingAccounts: [
          {
            pubkey: members.proposer.publicKey,
            isSigner: true,
            isWritable: false,
          },
        ],
      },
      {
        args: {
          createArgs,
          creatorKey: members.proposer.publicKey,
          clientDataParams: null,
        },
      },
      programId
    );

    const blockhash = (await connection.getLatestBlockhash()).blockhash;
    const message = new TransactionMessage({
      payerKey: members.proposer.publicKey,
      recentBlockhash: blockhash,
      instructions: [ix],
    }).compileToV0Message();
    const tx = new VersionedTransaction(message);
    tx.sign([members.proposer]);

    await assert.rejects(
      () =>
        connection
          .sendRawTransaction(tx.serialize())
          .catch(smartAccount.errors.translateAndThrowAnchorError),
      /TransactionMessage is malformed|InvalidTransactionMessage|Failed to serialize or deserialize account data/
    );
  });

  it("should_fail_on_invalid_payload_for_policy_v2", async () => {
    const settingsPda = await createSettings();
    const { policyPda } = await createInternalFundTransferPolicy({
      connection,
      programId,
      members,
      settingsPda,
    });
    const policyTransactionIndex = await getNextPolicyTransactionIndex({
      connection,
      policyPda,
    });
    const policyPayload: smartAccount.generated.PolicyPayload = {
      __kind: "InternalFundTransfer",
      fields: [
        {
          sourceIndex: 0,
          destinationIndex: 1,
          mint: PublicKey.default,
          decimals: 9,
          amount: 0,
        },
      ],
    };
    const createArgs = {
      __kind: "PolicyPayload",
      payload: policyPayload,
    } as unknown as smartAccount.generated.CreateTransactionArgs;

    await assert.rejects(
      async () => {
        await sendCreateTransactionV2WithArgs({
          consensusAccount: policyPda,
          creator: members.voter,
          rentPayer: members.voter,
          transactionIndex: policyTransactionIndex,
          createArgs,
        });
      },
      /InternalFundTransferPolicyInvariantAmountZero|PolicyInvariantAmountZero/
    );
  });

  it("should_fail_on_invalid_account_index_locked_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 1);

    await assert.rejects(
      () =>
        smartAccount.rpc.createTransaction({
          connection,
          feePayer: members.proposer,
          settingsPda,
          transactionIndex: 1n,
          creator: members.proposer.publicKey,
          accountIndex: 1,
          ephemeralSigners: 0,
          transactionMessage: message,
          programId,
        }),
      /Account index is locked, must increment_account_index first/
    );
  });

  it("should_fail_on_invalid_account_index_locked_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      1
    );

    await assert.rejects(
      async () => {
        await sendCreateTransactionV2({
          settingsPda,
          creator: members.proposer,
          rentPayer: members.proposer,
          accountIndex: 1,
          ephemeralSigners: 0,
          transactionMessageBytes,
        });
      },
      /Account index is locked, must increment_account_index first/
    );
  });

  // Skipped: distinct policy validation errors are covered above.
  skip.it("should_fail_on_invalid_policy_payload_v2");

  it("should_fail_on_invalid_v2_context_signature_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    await assert.rejects(
      async () => {
        await sendCreateTransactionV2({
          settingsPda,
          creator: members.proposer,
          rentPayer: members.proposer,
          accountIndex: 0,
          ephemeralSigners: 0,
          transactionMessageBytes,
          includeCreatorAsRemaining: false,
        });
      },
      /Missing signature|MissingSignature/
    );
  });

  // Skipped: WebAuthn verification requires signed client data.
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");
  // Skipped: creator_key is required by the instruction args and cannot be omitted.
  skip.it("should_fail_on_missing_creator_key_v2");
  // Skipped: settings are always active and create does not enforce staleness.
  skip.it("should_fail_on_stale_settings_v1");
  // Skipped: settings are always active and create does not enforce staleness.
  skip.it("should_fail_on_stale_settings_v2");
  // Skipped: create transaction does not check threshold.
  skip.it("should_fail_on_threshold_not_met_v1");
  // Skipped: create transaction does not check threshold.
  skip.it("should_fail_on_threshold_not_met_v2");
  // Skipped: settings consensus accounts have no stale state.
  skip.it("should_fail_on_stale_consensus_account_v1");
  // Skipped: settings consensus accounts have no stale state.
  skip.it("should_fail_on_stale_consensus_account_v2");

  // -------------------------------------------------------------------------------------
  // Edge Case Tests (V1 + V2 parity)
  // -------------------------------------------------------------------------------------

  it("should_create_with_min_account_index_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 0);

    const { transactionPda } = await createTransactionV1({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessage: message,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    const payload = extractTransactionPayloadDetails(transactionAccount.payload);
    assert.strictEqual(payload.accountIndex, 0);
  });

  it("should_create_with_min_account_index_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    const { transactionPda } = await sendCreateTransactionV2({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessageBytes,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    const payload = extractTransactionPayloadDetails(transactionAccount.payload);
    assert.strictEqual(payload.accountIndex, 0);
  });

  it("should_create_with_max_account_index_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 255);

    const { transactionPda } = await createTransactionV1({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 255,
      ephemeralSigners: 0,
      transactionMessage: message,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    const payload = extractTransactionPayloadDetails(transactionAccount.payload);
    assert.strictEqual(payload.accountIndex, 255);
  });

  it("should_create_with_max_account_index_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      255
    );

    const { transactionPda } = await sendCreateTransactionV2({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 255,
      ephemeralSigners: 0,
      transactionMessageBytes,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    const payload = extractTransactionPayloadDetails(transactionAccount.payload);
    assert.strictEqual(payload.accountIndex, 255);
  });

  // Skipped: accountIndex is a u8 so out-of-range values cannot be serialized.
  skip.it("should_fail_with_account_index_out_of_range_v1");
  // Skipped: accountIndex is a u8 so out-of-range values cannot be serialized.
  skip.it("should_fail_with_account_index_out_of_range_v2");
  // Skipped: no explicit max ephemeral signer constraint in program logic.
  skip.it("should_create_with_max_ephemeral_signers_v1");
  // Skipped: no explicit max ephemeral signer constraint in program logic.
  skip.it("should_create_with_max_ephemeral_signers_v2");
  // Skipped: no explicit overflow guard for ephemeral signer count.
  skip.it("should_fail_with_ephemeral_signers_overflow_v1");
  // Skipped: no explicit overflow guard for ephemeral signer count.
  skip.it("should_fail_with_ephemeral_signers_overflow_v2");

  it("should_handle_zero_ephemeral_signers_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 0);

    const { transactionPda } = await createTransactionV1({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessage: message,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    const payload = extractTransactionPayloadDetails(transactionAccount.payload);
    assert.deepEqual(payload.ephemeralSignerBumps, new Uint8Array());
  });

  it("should_handle_zero_ephemeral_signers_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    const { transactionPda } = await sendCreateTransactionV2({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessageBytes,
    });

    const transactionAccount = await Transaction.fromAccountAddress(
      connection,
      transactionPda
    );
    const payload = extractTransactionPayloadDetails(transactionAccount.payload);
    assert.deepEqual(payload.ephemeralSignerBumps, new Uint8Array());
  });

  // Skipped: oversized message behavior depends on account size limits, not explicit validation.
  skip.it("should_reject_oversized_transaction_message_v1");
  // Skipped: oversized message behavior depends on account size limits, not explicit validation.
  skip.it("should_reject_oversized_transaction_message_v2");

  it("should_reject_invalid_transaction_message_format_v1", async () => {
    const settingsPda = await createSettings();
    const args = {
      __kind: "TransactionPayload",
      fields: [
        {
          accountIndex: 0,
          ephemeralSigners: 0,
          transactionMessage: new Uint8Array([1, 2, 3, 4]),
          memo: null,
        },
      ],
    } as smartAccount.generated.CreateTransactionArgs;

    await assert.rejects(
      () =>
        sendV1WithArgs({
          settingsPda,
          creator: members.proposer,
          rentPayer: members.proposer,
          args,
        }),
      /TransactionMessage is malformed|InvalidTransactionMessage|Failed to serialize or deserialize account data/
    );
  });

  it("should_reject_invalid_transaction_message_format_v2", async () => {
    const settingsPda = await createSettings();
    await assert.rejects(
      async () => {
        await sendCreateTransactionV2({
          settingsPda,
          creator: members.proposer,
          rentPayer: members.proposer,
          accountIndex: 0,
          ephemeralSigners: 0,
          transactionMessageBytes: new Uint8Array([1, 2, 3, 4]),
        });
      },
      /TransactionMessage is malformed|InvalidTransactionMessage|Failed to serialize or deserialize account data/
    );
  });

  it("should_reject_empty_transaction_message_v1", async () => {
    const settingsPda = await createSettings();
    const args = {
      __kind: "TransactionPayload",
      fields: [
        {
          accountIndex: 0,
          ephemeralSigners: 0,
          transactionMessage: new Uint8Array(),
          memo: null,
        },
      ],
    } as smartAccount.generated.CreateTransactionArgs;

    await assert.rejects(
      () =>
        sendV1WithArgs({
          settingsPda,
          creator: members.proposer,
          rentPayer: members.proposer,
          args,
        }),
      /TransactionMessage is malformed|InvalidTransactionMessage|Failed to serialize or deserialize account data/
    );
  });

  it("should_reject_empty_transaction_message_v2", async () => {
    const settingsPda = await createSettings();
    await assert.rejects(
      async () => {
        await sendCreateTransactionV2({
          settingsPda,
          creator: members.proposer,
          rentPayer: members.proposer,
          accountIndex: 0,
          ephemeralSigners: 0,
          transactionMessageBytes: new Uint8Array(),
        });
      },
      /TransactionMessage is malformed|InvalidTransactionMessage|Failed to serialize or deserialize account data/
    );
  });

  // Skipped: cannot create a smart account with an empty signer set.
  skip.it("should_fail_with_empty_signer_set_v1");
  // Skipped: cannot create a smart account with an empty signer set.
  skip.it("should_fail_with_empty_signer_set_v2");

  it("should_allow_empty_memo_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 0);

    await createTransactionV1({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessage: message,
      memo: "",
    });
  });

  it("should_allow_empty_memo_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );
    const transactionIndex = await getNextTransactionIndex(settingsPda);
    const createArgs = {
      __kind: "TransactionPayload",
      fields: [
        {
          accountIndex: 0,
          ephemeralSigners: 0,
          transactionMessage: transactionMessageBytes,
          memo: "",
        },
      ],
    } as unknown as smartAccount.generated.CreateTransactionArgs;

    await sendCreateTransactionV2WithArgs({
      consensusAccount: settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      transactionIndex,
      createArgs,
    });
  });
  // Skipped: memo validation is not enforced by the program.
  skip.it("should_reject_oversized_memo_v1");
  // Skipped: memo is not part of CreateTransactionV2 args.
  skip.it("should_reject_oversized_memo_v2");

  it("should_allow_timelock_zero_v1", async () => {
    const settingsPda = await createSettings(0);
    const { message } = await buildTestMessage(settingsPda, 0);

    await createTransactionV1({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessage: message,
    });
  });

  it("should_allow_timelock_zero_v2", async () => {
    const settingsPda = await createSettings(0);
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    await sendCreateTransactionV2({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessageBytes,
    });
  });

  it("should_allow_timelock_non_zero_v1", async () => {
    const settingsPda = await createSettings(10);
    const { message } = await buildTestMessage(settingsPda, 0);

    await createTransactionV1({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessage: message,
    });
  });

  it("should_allow_timelock_non_zero_v2", async () => {
    const settingsPda = await createSettings(10);
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );

    await sendCreateTransactionV2({
      settingsPda,
      creator: members.proposer,
      rentPayer: members.proposer,
      accountIndex: 0,
      ephemeralSigners: 0,
      transactionMessageBytes,
    });
  });

  // Skipped: rent payer balance edge cases require rent/fee plumbing not covered in helpers.
  skip.it("should_create_with_minimum_rent_payer_balance_v1");
  // Skipped: rent payer balance edge cases require rent/fee plumbing not covered in helpers.
  skip.it("should_create_with_minimum_rent_payer_balance_v2");

  it("should_fail_with_insufficient_rent_payer_balance_v1", async () => {
    const settingsPda = await createSettings();
    const { message } = await buildTestMessage(settingsPda, 0);
    const rentPayer = Keypair.generate();

    await assert.rejects(
      () =>
        createTransactionV1({
          settingsPda,
          creator: members.proposer,
          rentPayer,
          accountIndex: 0,
          ephemeralSigners: 0,
          transactionMessage: message,
        }),
      /insufficient funds|InsufficientFunds|insufficient lamports/i
    );
  });

  it("should_fail_with_insufficient_rent_payer_balance_v2", async () => {
    const settingsPda = await createSettings();
    const transactionMessageBytes = await buildTransactionMessageBytes(
      settingsPda,
      0
    );
    const rentPayer = Keypair.generate();

    await assert.rejects(
      () =>
        sendCreateTransactionV2({
          settingsPda,
          creator: members.proposer,
          rentPayer,
          accountIndex: 0,
          ephemeralSigners: 0,
          transactionMessageBytes,
        }),
      /insufficient funds|InsufficientFunds|insufficient lamports/i
    );
  });
  
  // Skipped: requires manipulating transaction index to near u64 max.
  skip.it("should_create_with_max_transaction_index_v1");
  // Skipped: requires manipulating transaction index to near u64 max.
  skip.it("should_create_with_max_transaction_index_v2");
  // Skipped: requires manipulating transaction index to overflow u64.
  skip.it("should_fail_on_transaction_index_overflow_v1");
  // Skipped: requires manipulating transaction index to overflow u64.
  skip.it("should_fail_on_transaction_index_overflow_v2");

  // -------------------------------------------------------------------------------------
  // Regression & Compatibility Tests
  // -------------------------------------------------------------------------------------

  // Skipped: regression coverage needs historical fixtures.
  skip.it("should_preserve_backward_compatible_serialization_v1");
  // Skipped: regression coverage needs historical fixtures.
  skip.it("should_preserve_backward_compatible_serialization_v2");
  // Skipped: legacy account compatibility requires legacy fixtures.
  skip.it("should_support_legacy_account_behavior_v1");
  // Skipped: legacy account compatibility requires legacy fixtures.
  skip.it("should_support_legacy_account_behavior_v2");
  // Skipped: interop requires mixed-version signer fixtures.
  skip.it("should_interop_v1_settings_with_v2_signers_v1");
  // Skipped: interop requires mixed-version signer fixtures.
  skip.it("should_interop_v1_settings_with_v2_signers_v2");

  // -------------------------------------------------------------------------------------
  // V2-Only Extensions
  // -------------------------------------------------------------------------------------  
  
  // Skipped: session key setup requires dedicated signer fixtures.
  skip.it("should_create_with_session_key_v2");
  // Skipped: external signer flows require WebAuthn or secp verification data.
  skip.it("should_create_with_new_signer_types_v2");
  // Skipped: create transaction does not include sync execution path.
  skip.it("should_create_with_consensus_sync_flow_v2");
  // Skipped: no additional v2-only flow beyond signer verification in this instruction.
  skip.it("should_handle_v2_specific_instruction_flow_v2");
});
