import * as smartAccount from "@sqds/smart-account";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import assert from "assert";
import {
  createLocalhostConnection,
  generateFundedKeypair,
  getTestProgramId,
  getTestProgramTreasury,
  getNextAccountIndex,
  generateEd25519ExternalKeypair,
  signEd25519External,
  buildEd25519PrecompileInstruction,
  buildVoteMessage,
} from "../../../utils";
import {
  ExtraVerificationDataKind,
  serializeSingleExtraVerificationData,
  serializeExtraVerificationDataVec,
} from "../../../helpers/extraVerificationData";
import { sha256 } from "@noble/hashes/sha256";

const { Settings, Proposal } = smartAccount.accounts;
const { Permissions, Permission } = smartAccount.types;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

/** Build the sync consensus message matching on-chain create_sync_consensus_message + nonce.
 *  payloadHash binds the signature to the specific instructions being executed. */
function buildSyncConsensusMessage(
  settingsPda: PublicKey,
  transactionIndex: bigint,
  nextNonce: bigint,
  payloadHash: Uint8Array
): Uint8Array {
  const txIndexBytes = Buffer.alloc(8);
  txIndexBytes.writeBigUInt64LE(transactionIndex);
  const nonceBytes = Buffer.alloc(8);
  nonceBytes.writeBigUInt64LE(nextNonce);

  return sha256(
    Buffer.concat([
      Buffer.from("squads-sync", "utf-8"),
      settingsPda.toBuffer(),
      txIndexBytes,
      Buffer.from(payloadHash),
      nonceBytes,
    ])
  );
}

/** Patch EVD: replace trailing Option::None (0x00) with Option::Some + raw EVD bytes */
function patchEvd(
  ix: { data: Buffer },
  evdBytes: Uint8Array
): void {
  const withoutNull = ix.data.subarray(0, ix.data.length - 1);
  ix.data = Buffer.concat([
    withoutNull,
    Buffer.from([0x01]),
    Buffer.from(evdBytes),
  ]);
}

/**
 * Send a transaction with skipPreflight and check for failure.
 * Returns the error or null if it succeeded.
 */
async function sendAndExpectFailure(
  tx: VersionedTransaction,
  expectedErrorSubstring?: string
): Promise<{ err: any; logs: string[] }> {
  const signature = await connection.sendRawTransaction(tx.serialize(), {
    skipPreflight: true,
  });
  await connection.confirmTransaction(signature);

  const txResult = await connection.getTransaction(signature, {
    commitment: "confirmed",
    maxSupportedTransactionVersion: 0,
  });

  const logs = txResult?.meta?.logMessages || [];

  if (!txResult?.meta?.err) {
    throw new Error(
      `Expected transaction to fail but it succeeded.\nLogs: ${logs.join("\n")}`
    );
  }

  if (expectedErrorSubstring) {
    const logsJoined = logs.join("\n");
    assert.ok(
      logsJoined.includes(expectedErrorSubstring),
      `Expected error containing "${expectedErrorSubstring}" but got:\n${logsJoined}`
    );
  }

  return { err: txResult.meta.err, logs };
}

/**
 * Helper: create a smart account with Ed25519External signers and a native signer.
 * Returns settings PDA, native signer keypair, and all ed25519 keypairs.
 */
async function createAccountWithEd25519Signers(opts: {
  numEd25519Signers: number;
  threshold: number;
  ed25519Permissions?: number;
  includeNativeSigner?: boolean;
  nativePermissions?: number;
}): Promise<{
  settingsPda: PublicKey;
  creator: Keypair;
  nativeSigner: Keypair | null;
  ed25519Keypairs: { publicKey: Uint8Array; privateKey: Uint8Array }[];
}> {
  const creator = await generateFundedKeypair(connection);
  const treasury = getTestProgramTreasury();
  const accountIndex = await getNextAccountIndex(connection, programId);
  const [settingsPda] = smartAccount.getSettingsPda({ accountIndex, programId });

  const ed25519Keypairs = Array.from({ length: opts.numEd25519Signers }, () =>
    generateEd25519ExternalKeypair()
  );

  const signers: smartAccount.generated.SmartAccountSigner[] = [];

  let nativeSigner: Keypair | null = null;
  if (opts.includeNativeSigner !== false) {
    nativeSigner = await generateFundedKeypair(connection);
    signers.push({
      __kind: "Native",
      key: nativeSigner.publicKey,
      permissions: {
        mask:
          opts.nativePermissions ??
          (Permission.Initiate | Permission.Vote | Permission.Execute),
      },
    });
  }

  for (const kp of ed25519Keypairs) {
    signers.push({
      __kind: "Ed25519External",
      permissions: { mask: opts.ed25519Permissions ?? Permissions.all().mask },
      data: {
        externalPubkey: Array.from(kp.publicKey),
        sessionKeyData: { key: PublicKey.default, expiration: 0 },
      },
      nonce: 0,
    });
  }

  const sig = await smartAccount.rpc.createSmartAccount({
    connection,
    treasury,
    creator,
    settings: settingsPda,
    settingsAuthority: null,
    threshold: opts.threshold,
    signers,
    timeLock: 0,
    rentCollector: null,
    programId,
  });
  await connection.confirmTransaction(sig);

  return { settingsPda, creator, nativeSigner, ed25519Keypairs };
}

/**
 * Helper: create a settings transaction + proposal using the native signer.
 * Returns proposal PDA and transaction index.
 */
async function createSettingsTxAndProposal(
  settingsPda: PublicKey,
  nativeSigner: Keypair,
  transactionIndex: bigint
): Promise<{ proposalPda: PublicKey; transactionIndex: bigint }> {
  let sig = await smartAccount.rpc.createSettingsTransaction({
    connection,
    feePayer: nativeSigner,
    settingsPda,
    transactionIndex,
    creator: nativeSigner.publicKey,
    actions: [{ __kind: "ChangeThreshold", newThreshold: 1 }],
    programId,
  });
  await connection.confirmTransaction(sig);

  sig = await smartAccount.rpc.createProposal({
    connection,
    feePayer: nativeSigner,
    settingsPda,
    transactionIndex,
    creator: nativeSigner,
    programId,
  });
  await connection.confirmTransaction(sig);

  const [proposalPda] = smartAccount.getProposalPda({
    settingsPda,
    transactionIndex,
    programId,
  });

  return { proposalPda, transactionIndex };
}

/**
 * Helper: build an approve instruction with Ed25519 precompile verification.
 * Returns the precompile instruction and approve instruction pair.
 */
function buildEd25519ApproveInstructions(opts: {
  settingsPda: PublicKey;
  proposalPda: PublicKey;
  transactionIndex: bigint;
  ed25519Keypair: { publicKey: Uint8Array; privateKey: Uint8Array };
  nextNonce: bigint;
  overrideMessage?: Uint8Array;
  overrideSignerKeypair?: { publicKey: Uint8Array; privateKey: Uint8Array };
}): {
  precompileIx: ReturnType<typeof buildEd25519PrecompileInstruction>;
  approveIx: ReturnType<
    typeof smartAccount.generated.createApproveProposalV2Instruction
  >;
} {
  const hashedMessage =
    opts.overrideMessage ??
    buildVoteMessage(
      opts.proposalPda,
      0,
      opts.transactionIndex,
      opts.nextNonce
    );

  // Sign with the override keypair if provided (for wrong-key test)
  const signingKeypair = opts.overrideSignerKeypair ?? opts.ed25519Keypair;
  const sig = signEd25519External(hashedMessage, signingKeypair.privateKey);

  // Build precompile instruction with the signing key's pubkey
  const precompileIx = buildEd25519PrecompileInstruction(
    sig,
    signingKeypair.publicKey,
    hashedMessage
  );

  // The signer key in the approve instruction is always the registered signer's key
  const signerKey = new PublicKey(opts.ed25519Keypair.publicKey);
  const evdBytes = serializeSingleExtraVerificationData({
    kind: ExtraVerificationDataKind.Ed25519Precompile,
  });

  const approveIx =
    smartAccount.generated.createApproveProposalV2Instruction(
      {
        consensusAccount: opts.settingsPda,
        signer: signerKey,
        proposal: opts.proposalPda,
        program: programId,
        anchorRemainingAccounts: [
          {
            pubkey: SYSVAR_INSTRUCTIONS_PUBKEY,
            isSigner: false,
            isWritable: false,
          },
        ],
      },
      { args: { memo: null }, extraVerificationData: null },
      programId
    );
  patchEvd(approveIx, evdBytes);

  return { precompileIx, approveIx };
}

describe("Instructions / external_signer_security", () => {
  // ---------------------------------------------------------------
  // Test 1: Nonce replay attack — same signer cannot approve twice
  // ---------------------------------------------------------------
  it("nonce replay attack — same signer cannot approve same proposal twice", async () => {
    // Use threshold=2 so one approve doesn't finalize the proposal
    const { settingsPda, creator, nativeSigner, ed25519Keypairs } =
      await createAccountWithEd25519Signers({
        numEd25519Signers: 1,
        threshold: 2,
        includeNativeSigner: true,
        nativePermissions: Permission.Initiate | Permission.Vote | Permission.Execute,
      });
    const ed25519Kp = ed25519Keypairs[0];

    await new Promise((resolve) => setTimeout(resolve, 1000));

    // Create settings tx + proposal
    const { proposalPda, transactionIndex } =
      await createSettingsTxAndProposal(settingsPda, nativeSigner!, 1n);

    // Successfully approve the proposal with the external signer (nonce 0 -> 1)
    const { precompileIx, approveIx } = buildEd25519ApproveInstructions({
      settingsPda,
      proposalPda,
      transactionIndex,
      ed25519Keypair: ed25519Kp,
      nextNonce: 1n, // current nonce is 0, next is 1
    });

    let message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx, approveIx],
    }).compileToV0Message();

    let tx = new VersionedTransaction(message);
    tx.sign([creator]);

    const sig1 = await connection.sendRawTransaction(tx.serialize(), {
      skipPreflight: true,
    });
    await connection.confirmTransaction(sig1);
    const txResult1 = await connection.getTransaction(sig1, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    if (txResult1?.meta?.err) {
      console.error("First approve TX logs:", txResult1.meta.logMessages);
      throw new Error(
        `First approve TX failed: ${JSON.stringify(txResult1.meta.err)}`
      );
    }

    // Verify first approval was recorded
    const proposal = await Proposal.fromAccountAddress(connection, proposalPda);
    assert.ok(proposal.approved.length > 0, "Proposal should have 1 approval");

    // REPLAY ATTACK: try to approve AGAIN with the OLD nonce (1) instead of new nonce (2)
    // On-chain nonce is now 1, so expected next_nonce = 2. Using next_nonce = 1 is stale.
    const replayMessage = buildVoteMessage(proposalPda, 0, transactionIndex, 1n);
    const replaySig = signEd25519External(replayMessage, ed25519Kp.privateKey);
    const replayPrecompileIx = buildEd25519PrecompileInstruction(
      replaySig,
      ed25519Kp.publicKey,
      replayMessage
    );

    const signerKey = new PublicKey(ed25519Kp.publicKey);
    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.Ed25519Precompile,
    });
    const replayApproveIx =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey,
          proposal: proposalPda,
          program: programId,
          anchorRemainingAccounts: [
            {
              pubkey: SYSVAR_INSTRUCTIONS_PUBKEY,
              isSigner: false,
              isWritable: false,
            },
          ],
        },
        { args: { memo: null }, extraVerificationData: null },
        programId
      );
    patchEvd(replayApproveIx, evdBytes);

    message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [replayPrecompileIx, replayApproveIx],
    }).compileToV0Message();

    tx = new VersionedTransaction(message);
    tx.sign([creator]);

    // Should fail: either AlreadyApproved (signer already voted) or
    // PrecompileMessageMismatch (nonce stale, message hash doesn't match)
    await sendAndExpectFailure(tx);
  });

  // ---------------------------------------------------------------
  // Test 2: Wrong key signature — valid signature from non-member
  // ---------------------------------------------------------------
  it("wrong key signature — valid signature from non-member rejected", async () => {
    const { settingsPda, creator, nativeSigner, ed25519Keypairs } =
      await createAccountWithEd25519Signers({
        numEd25519Signers: 1,
        threshold: 1,
        includeNativeSigner: true,
      });
    const ed25519Kp = ed25519Keypairs[0]; // registered signer

    await new Promise((resolve) => setTimeout(resolve, 1000));

    const { proposalPda, transactionIndex } = await createSettingsTxAndProposal(
      settingsPda,
      nativeSigner!,
      1n
    );

    // Generate a different keypair (non-member)
    const wrongKeypair = generateEd25519ExternalKeypair();

    // Sign correct message with the wrong key
    const hashedMessage = buildVoteMessage(
      proposalPda,
      0,
      transactionIndex,
      1n
    );
    const wrongSig = signEd25519External(hashedMessage, wrongKeypair.privateKey);

    // Build precompile instruction with wrong key's signature and pubkey
    const precompileIx = buildEd25519PrecompileInstruction(
      wrongSig,
      wrongKeypair.publicKey, // wrong public key
      hashedMessage
    );

    // The approve instruction still references the registered signer key
    const signerKey = new PublicKey(ed25519Kp.publicKey);
    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.Ed25519Precompile,
    });

    const approveIx =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey,
          proposal: proposalPda,
          program: programId,
          anchorRemainingAccounts: [
            {
              pubkey: SYSVAR_INSTRUCTIONS_PUBKEY,
              isSigner: false,
              isWritable: false,
            },
          ],
        },
        { args: { memo: null }, extraVerificationData: null },
        programId
      );
    patchEvd(approveIx, evdBytes);

    const message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx, approveIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([creator]);

    // Should fail: precompile pubkey doesn't match stored signer pubkey
    await sendAndExpectFailure(tx, "PrecompileMessageMismatch");
  });

  // ---------------------------------------------------------------
  // Test 3: Wrong message — sign reject but submit as approve
  // ---------------------------------------------------------------
  it("wrong message — sign reject but submit as approve", async () => {
    const { settingsPda, creator, nativeSigner, ed25519Keypairs } =
      await createAccountWithEd25519Signers({
        numEd25519Signers: 1,
        threshold: 1,
        includeNativeSigner: true,
      });
    const ed25519Kp = ed25519Keypairs[0];

    await new Promise((resolve) => setTimeout(resolve, 1000));

    const { proposalPda, transactionIndex } = await createSettingsTxAndProposal(
      settingsPda,
      nativeSigner!,
      1n
    );

    // Sign a REJECT message (vote=1) ...
    const rejectMessage = buildVoteMessage(
      proposalPda,
      1, // vote=1 = Reject
      transactionIndex,
      1n
    );
    const sig = signEd25519External(rejectMessage, ed25519Kp.privateKey);

    // ... but build precompile instruction with reject-signed message
    const precompileIx = buildEd25519PrecompileInstruction(
      sig,
      ed25519Kp.publicKey,
      rejectMessage // precompile verifies this reject-message signature
    );

    // ... and submit it as an APPROVE instruction
    // On-chain will compute the APPROVE message (vote=0) and compare
    const signerKey = new PublicKey(ed25519Kp.publicKey);
    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.Ed25519Precompile,
    });

    const approveIx =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey,
          proposal: proposalPda,
          program: programId,
          anchorRemainingAccounts: [
            {
              pubkey: SYSVAR_INSTRUCTIONS_PUBKEY,
              isSigner: false,
              isWritable: false,
            },
          ],
        },
        { args: { memo: null }, extraVerificationData: null },
        programId
      );
    patchEvd(approveIx, evdBytes);

    const message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx, approveIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([creator]);

    // Should fail: on-chain expects sha256(...vote=0...) but precompile has sha256(...vote=1...)
    await sendAndExpectFailure(tx, "PrecompileMessageMismatch");
  });

  // ---------------------------------------------------------------
  // Test 4: Permission enforcement — initiate-only signer cannot vote
  // ---------------------------------------------------------------
  it("permission enforcement — initiate-only signer cannot vote", async () => {
    const creator = await generateFundedKeypair(connection);
    const nativeSigner = await generateFundedKeypair(connection);
    const ed25519Kp = generateEd25519ExternalKeypair();
    const treasury = getTestProgramTreasury();

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    // External signer has ONLY Initiate permission
    const signers: smartAccount.generated.SmartAccountSigner[] = [
      {
        __kind: "Native",
        key: nativeSigner.publicKey,
        permissions: {
          mask: Permission.Initiate | Permission.Vote | Permission.Execute,
        },
      },
      {
        __kind: "Ed25519External",
        permissions: { mask: Permission.Initiate }, // Initiate only!
        data: {
          externalPubkey: Array.from(ed25519Kp.publicKey),
          sessionKeyData: { key: PublicKey.default, expiration: 0 },
        },
        nonce: 0,
      },
    ];

    const createSig = await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers,
      timeLock: 0,
      rentCollector: null,
      programId,
    });
    await connection.confirmTransaction(createSig);

    await new Promise((resolve) => setTimeout(resolve, 1000));

    // Create settings tx + proposal with native signer
    const { proposalPda, transactionIndex } = await createSettingsTxAndProposal(
      settingsPda,
      nativeSigner,
      1n
    );

    // External signer tries to approve (requires Vote permission)
    const { precompileIx, approveIx } = buildEd25519ApproveInstructions({
      settingsPda,
      proposalPda,
      transactionIndex,
      ed25519Keypair: ed25519Kp,
      nextNonce: 1n,
    });

    const message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx, approveIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([creator]);

    // Should fail: signer only has Initiate permission, not Vote
    await sendAndExpectFailure(tx, "Unauthorized");
  });

  // ---------------------------------------------------------------
  // Test 5: Threshold enforcement — insufficient external signatures in sync path
  // ---------------------------------------------------------------
  it("threshold enforcement — insufficient external signatures in sync path", async () => {
    // Create smart account with 3 Ed25519External signers, threshold 2
    const { settingsPda, creator, nativeSigner: _, ed25519Keypairs } =
      await createAccountWithEd25519Signers({
        numEd25519Signers: 3,
        threshold: 2,
        includeNativeSigner: false,
        ed25519Permissions: Permission.Initiate | Permission.Vote | Permission.Execute,
      });

    await new Promise((resolve) => setTimeout(resolve, 1000));

    const settings = await Settings.fromAccountAddress(connection, settingsPda);
    assert.strictEqual(settings.signers.length, 3);
    assert.strictEqual(settings.threshold, 2);

    // Fund the vault
    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });
    await connection.confirmTransaction(
      await connection.requestAirdrop(vaultPda, 2 * LAMPORTS_PER_SOL)
    );

    // Build a simple transfer instruction
    const transferAmount = 1_000_000;
    const receiver = Keypair.generate();
    const transferIx = SystemProgram.transfer({
      fromPubkey: vaultPda,
      toPubkey: receiver.publicKey,
      lamports: transferAmount,
    });

    const { instructions: rawCompiledIxBytes, accounts: txAccounts } =
      smartAccount.utils.instructionsToSynchronousTransactionDetailsV2({
        vaultPda,
        members: [],
        transaction_instructions: [transferIx],
      });

    const compiledIxBytes = Buffer.from(rawCompiledIxBytes);

    // Hash the SyncPayload::Transaction(Vec<u8>) Borsh serialization
    const payloadLenBytes = Buffer.alloc(4);
    payloadLenBytes.writeUInt32LE(compiledIxBytes.length);
    const payloadHash = sha256(
      Buffer.concat([Buffer.from([0x00]), payloadLenBytes, compiledIxBytes])
    );

    // Build sync message (only 1 signer instead of threshold=2)
    const transactionIndex = BigInt(settings.transactionIndex.toString());

    // Each signer has nonce 0, so next_nonce = 1
    const hashedMessage = buildSyncConsensusMessage(
      settingsPda,
      transactionIndex,
      1n,
      payloadHash
    );

    // Sign with only 1 external signer
    const ed25519Sig = signEd25519External(
      hashedMessage,
      ed25519Keypairs[0].privateKey
    );

    // Build EVD for only 1 syscall signer
    const extraVerificationData = serializeExtraVerificationDataVec([
      {
        kind: ExtraVerificationDataKind.Ed25519Syscall,
        signature: ed25519Sig,
      },
    ]);

    const ed25519SignerKey = new PublicKey(ed25519Keypairs[0].publicKey);

    // remaining_accounts: [signer(1), txAccounts...]
    // num_signers = 1 (only 1 signer provided)
    const allRemainingAccounts = [
      { pubkey: ed25519SignerKey, isSigner: false, isWritable: false },
      ...txAccounts,
    ];

    const syncIx =
      smartAccount.generated.createExecuteTransactionSyncV2ExternalInstruction(
        {
          consensusAccount: settingsPda,
          program: programId,
          anchorRemainingAccounts: allRemainingAccounts,
        },
        {
          args: {
            accountIndex: 0,
            numSigners: 1, // Only 1 signer (threshold is 2)
            payload: {
              __kind: "Transaction",
              fields: [compiledIxBytes],
            },
          },
          extraVerificationData: null,
        },
        programId
      );

    // Patch EVD
    const withoutNull = syncIx.data.subarray(0, syncIx.data.length - 1);
    syncIx.data = Buffer.concat([
      withoutNull,
      Buffer.from([0x01]),
      Buffer.from(extraVerificationData),
    ]);

    const { blockhash } = await connection.getLatestBlockhash();
    const message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: blockhash,
      instructions: [syncIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([creator]);

    // Should fail: only 1 signer but threshold is 2
    await sendAndExpectFailure(tx, "InvalidSignerCount");
  });

  // ---------------------------------------------------------------
  // Test 6: Duplicate external signature detection in sync path
  // ---------------------------------------------------------------
  it("duplicate external signature detection in sync path", async () => {
    // Create smart account with 2 Ed25519External signers, threshold 2
    const { settingsPda, creator, nativeSigner: _, ed25519Keypairs } =
      await createAccountWithEd25519Signers({
        numEd25519Signers: 2,
        threshold: 2,
        includeNativeSigner: false,
        ed25519Permissions: Permission.Initiate | Permission.Vote | Permission.Execute,
      });

    await new Promise((resolve) => setTimeout(resolve, 1000));

    const settings = await Settings.fromAccountAddress(connection, settingsPda);
    assert.strictEqual(settings.signers.length, 2);
    assert.strictEqual(settings.threshold, 2);

    // Fund the vault
    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });
    await connection.confirmTransaction(
      await connection.requestAirdrop(vaultPda, 2 * LAMPORTS_PER_SOL)
    );

    // Build a simple transfer instruction
    const transferAmount = 1_000_000;
    const receiver = Keypair.generate();
    const transferIx = SystemProgram.transfer({
      fromPubkey: vaultPda,
      toPubkey: receiver.publicKey,
      lamports: transferAmount,
    });

    const { instructions: rawCompiledIxBytes, accounts: txAccounts } =
      smartAccount.utils.instructionsToSynchronousTransactionDetailsV2({
        vaultPda,
        members: [],
        transaction_instructions: [transferIx],
      });

    const compiledIxBytes = Buffer.from(rawCompiledIxBytes);

    // Hash the SyncPayload::Transaction(Vec<u8>) Borsh serialization
    const payloadLenBytes = Buffer.alloc(4);
    payloadLenBytes.writeUInt32LE(compiledIxBytes.length);
    const payloadHash = sha256(
      Buffer.concat([Buffer.from([0x00]), payloadLenBytes, compiledIxBytes])
    );

    const transactionIndex = BigInt(settings.transactionIndex.toString());

    // Both signers have nonce 0, next_nonce = 1
    const hashedMessage = buildSyncConsensusMessage(
      settingsPda,
      transactionIndex,
      1n,
      payloadHash
    );

    // Sign with the SAME signer twice (duplicate)
    const sig1 = signEd25519External(
      hashedMessage,
      ed25519Keypairs[0].privateKey
    );
    const sig2 = signEd25519External(
      hashedMessage,
      ed25519Keypairs[0].privateKey // same key!
    );

    // Build EVD with 2 syscall entries (both for the same signer)
    const extraVerificationData = serializeExtraVerificationDataVec([
      {
        kind: ExtraVerificationDataKind.Ed25519Syscall,
        signature: sig1,
      },
      {
        kind: ExtraVerificationDataKind.Ed25519Syscall,
        signature: sig2,
      },
    ]);
    const ed25519SignerKey = new PublicKey(ed25519Keypairs[0].publicKey);

    // Pass the SAME signer account twice in remaining_accounts
    const allRemainingAccounts = [
      { pubkey: ed25519SignerKey, isSigner: false, isWritable: false },
      { pubkey: ed25519SignerKey, isSigner: false, isWritable: false }, // duplicate!
      ...txAccounts,
    ];

    const syncIx =
      smartAccount.generated.createExecuteTransactionSyncV2ExternalInstruction(
        {
          consensusAccount: settingsPda,
          program: programId,
          anchorRemainingAccounts: allRemainingAccounts,
        },
        {
          args: {
            accountIndex: 0,
            numSigners: 2, // Claiming 2 signers but they're the same
            payload: {
              __kind: "Transaction",
              fields: [compiledIxBytes],
            },
          },
          extraVerificationData: null,
        },
        programId
      );

    // Patch EVD
    const withoutNull = syncIx.data.subarray(0, syncIx.data.length - 1);
    syncIx.data = Buffer.concat([
      withoutNull,
      Buffer.from([0x01]),
      Buffer.from(extraVerificationData),
    ]);

    const { blockhash } = await connection.getLatestBlockhash();
    const message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: blockhash,
      instructions: [syncIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([creator]);

    // Should fail: duplicate signer detected
    await sendAndExpectFailure(tx, "DuplicateSigner");
  });
});
