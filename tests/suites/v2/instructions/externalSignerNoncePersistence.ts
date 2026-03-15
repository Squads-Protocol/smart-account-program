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

/** Build the sync consensus message matching on-chain create_sync_consensus_message */
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
function patchEvd(ix: { data: Buffer }, evdBytes: Uint8Array): void {
  const withoutNull = ix.data.subarray(0, ix.data.length - 1);
  ix.data = Buffer.concat([
    withoutNull,
    Buffer.from([0x01]),
    Buffer.from(evdBytes),
  ]);
}

/** Send a transaction with skipPreflight and assert it succeeds. */
async function sendAndConfirm(
  tx: VersionedTransaction,
  label: string
): Promise<string> {
  const signature = await connection.sendRawTransaction(tx.serialize(), {
    skipPreflight: true,
  });
  await connection.confirmTransaction(signature);

  const txResult = await connection.getTransaction(signature, {
    commitment: "confirmed",
    maxSupportedTransactionVersion: 0,
  });
  if (txResult?.meta?.err) {
    throw new Error(
      `${label} failed: ${JSON.stringify(txResult.meta.err)}\nLogs: ${txResult.meta.logMessages?.join("\n")}`
    );
  }
  return signature;
}

/** Send a transaction with skipPreflight and assert it fails. */
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

/** Create a smart account with 1 native signer + N Ed25519External signers. */
async function createAccountWithSigners(opts: {
  numEd25519Signers: number;
  threshold: number;
}): Promise<{
  settingsPda: PublicKey;
  creator: Keypair;
  nativeSigner: Keypair;
  ed25519Keypairs: { publicKey: Uint8Array; privateKey: Uint8Array }[];
}> {
  const creator = await generateFundedKeypair(connection);
  const nativeSigner = await generateFundedKeypair(connection);
  const treasury = getTestProgramTreasury();
  const accountIndex = await getNextAccountIndex(connection, programId);
  const [settingsPda] = smartAccount.getSettingsPda({
    accountIndex,
    programId,
  });

  const ed25519Keypairs = Array.from({ length: opts.numEd25519Signers }, () =>
    generateEd25519ExternalKeypair()
  );

  const signers: smartAccount.generated.SmartAccountSigner[] = [
    {
      __kind: "Native",
      key: nativeSigner.publicKey,
      permissions: { mask: Permissions.all().mask },
    },
    ...ed25519Keypairs.map((kp) => ({
      __kind: "Ed25519External" as const,
      permissions: { mask: Permissions.all().mask },
      data: {
        externalPubkey: Array.from(kp.publicKey),
        sessionKeyData: { key: PublicKey.default, expiration: 0 },
      },
      nonce: 0,
    })),
  ];

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
 * Get the nonce of an Ed25519External signer from the Settings account.
 * Signers are sorted by key on-chain, so we search by public key rather than index.
 * If externalPubkey is provided, match on that; otherwise return the Nth Ed25519External.
 */
async function getEd25519SignerNonce(
  settingsPda: PublicKey,
  externalPubkey: Uint8Array
): Promise<bigint> {
  const settings = await Settings.fromAccountAddress(connection, settingsPda);
  const targetKey = new PublicKey(externalPubkey).toBase58();

  for (const signer of settings.signers as any[]) {
    if (signer.__kind === "Ed25519External") {
      // The signer's key() on-chain is the ed25519 pubkey interpreted as Pubkey
      const signerKey = new PublicKey(
        Uint8Array.from(signer.data.externalPubkey)
      ).toBase58();
      if (signerKey === targetKey) {
        return BigInt(signer.nonce.toString());
      }
    }
  }

  throw new Error(
    `Cannot find Ed25519External signer with pubkey ${targetKey} in settings. Signers: ${JSON.stringify(settings.signers)}`
  );
}

/**
 * Build and return a sync transfer transaction using Ed25519 syscall external signer.
 * Reads current Settings to get transactionIndex.
 */
async function buildSyncTransferTx(opts: {
  settingsPda: PublicKey;
  creator: Keypair;
  nativeSigner: Keypair;
  ed25519Keypair: { publicKey: Uint8Array; privateKey: Uint8Array };
  nextNonce: bigint;
  transferAmount?: number;
}): Promise<{ tx: VersionedTransaction; receiver: Keypair }> {
  const settings = await Settings.fromAccountAddress(
    connection,
    opts.settingsPda
  );
  const transactionIndex = BigInt(settings.transactionIndex.toString());

  const [vaultPda] = smartAccount.getSmartAccountPda({
    settingsPda: opts.settingsPda,
    accountIndex: 0,
    programId,
  });

  // Default to 1 SOL to avoid InsufficientFundsForRent on the receiver
  const transferAmount = opts.transferAmount ?? LAMPORTS_PER_SOL;
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

  // Hash SyncPayload::Transaction(Vec<u8>) Borsh serialization:
  // [0x00 (enum variant), 4-byte LE length, ...compiled_ix_bytes]
  const payloadLenBytes = Buffer.alloc(4);
  payloadLenBytes.writeUInt32LE(compiledIxBytes.length);
  const payloadHash = sha256(
    Buffer.concat([Buffer.from([0x00]), payloadLenBytes, compiledIxBytes])
  );

  const hashedMessage = buildSyncConsensusMessage(
    opts.settingsPda,
    transactionIndex,
    opts.nextNonce,
    payloadHash
  );

  // Sign with Ed25519 external (syscall path — signature in EVD)
  const ed25519Sig = signEd25519External(
    hashedMessage,
    opts.ed25519Keypair.privateKey
  );

  const extraVerificationData = serializeExtraVerificationDataVec([
    {
      kind: ExtraVerificationDataKind.Ed25519Syscall,
      signature: ed25519Sig,
    },
  ]);

  const ed25519SignerKey = new PublicKey(opts.ed25519Keypair.publicKey);

  // remaining_accounts: [native(isSigner=true), ed25519(isSigner=false), txAccounts...]
  const allRemainingAccounts = [
    {
      pubkey: opts.nativeSigner.publicKey,
      isSigner: true,
      isWritable: false,
    },
    { pubkey: ed25519SignerKey, isSigner: false, isWritable: false },
    ...txAccounts,
  ];

  const syncIx =
    smartAccount.generated.createExecuteTransactionSyncV2ExternalInstruction(
      {
        consensusAccount: opts.settingsPda,
        program: programId,
        anchorRemainingAccounts: allRemainingAccounts,
      },
      {
        args: {
          accountIndex: 0,
          numSigners: 2,
          payload: {
            __kind: "Transaction",
            fields: [compiledIxBytes],
          },
        },
        extraVerificationData: null,
      },
      programId
    );

  // Patch EVD onto instruction data
  patchEvd(syncIx, extraVerificationData);

  const { blockhash } = await connection.getLatestBlockhash();
  const message = new TransactionMessage({
    payerKey: opts.creator.publicKey,
    recentBlockhash: blockhash,
    instructions: [syncIx],
  }).compileToV0Message();

  const tx = new VersionedTransaction(message);
  tx.sign([opts.creator, opts.nativeSigner]);

  return { tx, receiver };
}

describe("Instructions / external_signer_nonce_persistence", () => {
  // ---------------------------------------------------------------
  // CRITICAL: Two sequential sync transactions — nonce 0 → 1 → 2
  // This is the #1 test that would have caught the missing `mut` bug.
  // ---------------------------------------------------------------
  it("nonce persists across two sequential sync transactions (0 → 1 → 2)", async () => {
    const { settingsPda, creator, nativeSigner, ed25519Keypairs } =
      await createAccountWithSigners({ numEd25519Signers: 1, threshold: 2 });
    const ed25519Kp = ed25519Keypairs[0];

    await new Promise((resolve) => setTimeout(resolve, 1000));

    // Verify initial nonce is 0
    const initialNonce = await getEd25519SignerNonce(settingsPda, ed25519Kp.publicKey);
    assert.strictEqual(initialNonce, 0n, "Initial nonce should be 0");

    // Fund the vault
    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });
    await connection.confirmTransaction(
      await connection.requestAirdrop(vaultPda, 5 * LAMPORTS_PER_SOL)
    );

    // --- Transaction #1: nonce 0 → 1 ---
    const { tx: tx1, receiver: receiver1 } = await buildSyncTransferTx({
      settingsPda,
      creator,
      nativeSigner,
      ed25519Keypair: ed25519Kp,
      nextNonce: 1n,
      transferAmount: LAMPORTS_PER_SOL,
    });

    await sendAndConfirm(tx1, "Sync TX #1");

    // Verify nonce incremented to 1
    const nonceAfterTx1 = await getEd25519SignerNonce(settingsPda, ed25519Kp.publicKey);
    assert.strictEqual(
      nonceAfterTx1,
      1n,
      "Nonce should be 1 after first sync TX"
    );

    // Verify transfer succeeded
    const balance1 = await connection.getBalance(receiver1.publicKey);
    assert.strictEqual(
      balance1,
      LAMPORTS_PER_SOL,
      "Receiver 1 should have received funds"
    );

    // --- Transaction #2: nonce 1 → 2 ---
    const { tx: tx2, receiver: receiver2 } = await buildSyncTransferTx({
      settingsPda,
      creator,
      nativeSigner,
      ed25519Keypair: ed25519Kp,
      nextNonce: 2n,
      transferAmount: LAMPORTS_PER_SOL,
    });

    await sendAndConfirm(tx2, "Sync TX #2");

    // Verify nonce incremented to 2
    const nonceAfterTx2 = await getEd25519SignerNonce(settingsPda, ed25519Kp.publicKey);
    assert.strictEqual(
      nonceAfterTx2,
      2n,
      "Nonce should be 2 after second sync TX"
    );

    // Verify second transfer succeeded
    const balance2 = await connection.getBalance(receiver2.publicKey);
    assert.strictEqual(
      balance2,
      LAMPORTS_PER_SOL,
      "Receiver 2 should have received funds"
    );
  });

  // ---------------------------------------------------------------
  // Sequential proposal votes — nonce increments in async path
  // ---------------------------------------------------------------
  it("nonce persists across two sequential proposal votes (0 → 1 → 2)", async () => {
    const { settingsPda, creator, nativeSigner, ed25519Keypairs } =
      await createAccountWithSigners({ numEd25519Signers: 1, threshold: 2 });
    const ed25519Kp = ed25519Keypairs[0];
    const signerKey = new PublicKey(ed25519Kp.publicKey);

    await new Promise((resolve) => setTimeout(resolve, 1000));

    // Verify initial nonce is 0
    assert.strictEqual(await getEd25519SignerNonce(settingsPda, ed25519Kp.publicKey), 0n);

    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.Ed25519Precompile,
    });

    // --- Proposal #1 ---
    let settings = await Settings.fromAccountAddress(connection, settingsPda);
    const txIndex1 = BigInt(settings.transactionIndex.toString()) + 1n;

    let sig = await smartAccount.rpc.createSettingsTransactionV2({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex: txIndex1,
      creator: nativeSigner.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
      programId,
    });
    await connection.confirmTransaction(sig);

    sig = await smartAccount.rpc.createProposalV2({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex: txIndex1,
      creator: nativeSigner,
      programId,
    });
    await connection.confirmTransaction(sig);

    const [proposalPda1] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: txIndex1,
      programId,
    });

    // External signer approves proposal #1 (nonce 0 → 1)
    const hashedMsg1 = buildVoteMessage(proposalPda1, 0, txIndex1, 1n);
    const edSig1 = signEd25519External(hashedMsg1, ed25519Kp.privateKey);
    const precompileIx1 = buildEd25519PrecompileInstruction(
      edSig1,
      ed25519Kp.publicKey,
      hashedMsg1
    );

    const approveIx1 =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey,
          proposal: proposalPda1,
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
    patchEvd(approveIx1, evdBytes);

    let message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx1, approveIx1],
    }).compileToV0Message();
    let tx = new VersionedTransaction(message);
    tx.sign([creator]);

    await sendAndConfirm(tx, "Approve proposal #1");

    // Verify nonce = 1
    assert.strictEqual(
      await getEd25519SignerNonce(settingsPda, ed25519Kp.publicKey),
      1n,
      "Nonce should be 1 after first approval"
    );

    // --- Proposal #2 ---
    settings = await Settings.fromAccountAddress(connection, settingsPda);
    const txIndex2 = BigInt(settings.transactionIndex.toString()) + 1n;

    sig = await smartAccount.rpc.createSettingsTransactionV2({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex: txIndex2,
      creator: nativeSigner.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
      programId,
    });
    await connection.confirmTransaction(sig);

    sig = await smartAccount.rpc.createProposalV2({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex: txIndex2,
      creator: nativeSigner,
      programId,
    });
    await connection.confirmTransaction(sig);

    const [proposalPda2] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: txIndex2,
      programId,
    });

    // External signer approves proposal #2 (nonce 1 → 2)
    const hashedMsg2 = buildVoteMessage(proposalPda2, 0, txIndex2, 2n);
    const edSig2 = signEd25519External(hashedMsg2, ed25519Kp.privateKey);
    const precompileIx2 = buildEd25519PrecompileInstruction(
      edSig2,
      ed25519Kp.publicKey,
      hashedMsg2
    );

    const approveIx2 =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey,
          proposal: proposalPda2,
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
    patchEvd(approveIx2, evdBytes);

    message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx2, approveIx2],
    }).compileToV0Message();
    tx = new VersionedTransaction(message);
    tx.sign([creator]);

    await sendAndConfirm(tx, "Approve proposal #2");

    // Verify nonce = 2
    assert.strictEqual(
      await getEd25519SignerNonce(settingsPda, ed25519Kp.publicKey),
      2n,
      "Nonce should be 2 after second approval"
    );
  });

  // ---------------------------------------------------------------
  // Stale nonce rejection — using nonce=1 again after it was consumed
  // ---------------------------------------------------------------
  it("stale nonce rejected on second sync transaction", async () => {
    const { settingsPda, creator, nativeSigner, ed25519Keypairs } =
      await createAccountWithSigners({ numEd25519Signers: 1, threshold: 2 });
    const ed25519Kp = ed25519Keypairs[0];

    await new Promise((resolve) => setTimeout(resolve, 1000));

    // Fund the vault
    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });
    await connection.confirmTransaction(
      await connection.requestAirdrop(vaultPda, 5 * LAMPORTS_PER_SOL)
    );

    // First sync TX succeeds with nonce=1
    const { tx: tx1 } = await buildSyncTransferTx({
      settingsPda,
      creator,
      nativeSigner,
      ed25519Keypair: ed25519Kp,
      nextNonce: 1n,
    });
    await sendAndConfirm(tx1, "Sync TX #1 (nonce=1)");

    // Verify nonce is now 1
    assert.strictEqual(await getEd25519SignerNonce(settingsPda, ed25519Kp.publicKey), 1n);

    // Build second TX with STALE nonce=1 (should be 2)
    const settings = await Settings.fromAccountAddress(connection, settingsPda);
    const transactionIndex = BigInt(settings.transactionIndex.toString());

    const receiver = Keypair.generate();
    const transferIx = SystemProgram.transfer({
      fromPubkey: vaultPda,
      toPubkey: receiver.publicKey,
      lamports: LAMPORTS_PER_SOL,
    });

    const { instructions: rawCompiledIxBytes, accounts: txAccounts } =
      smartAccount.utils.instructionsToSynchronousTransactionDetailsV2({
        vaultPda,
        members: [],
        transaction_instructions: [transferIx],
      });

    const compiledIxBytes = Buffer.from(rawCompiledIxBytes);
    const payloadLenBytes = Buffer.alloc(4);
    payloadLenBytes.writeUInt32LE(compiledIxBytes.length);
    const payloadHash = sha256(
      Buffer.concat([Buffer.from([0x00]), payloadLenBytes, compiledIxBytes])
    );

    // Build message with STALE nonce=1 (on-chain nonce is now 1, expected next=2)
    const hashedMessage = buildSyncConsensusMessage(
      settingsPda,
      transactionIndex,
      1n, // STALE
      payloadHash
    );

    const ed25519Sig = signEd25519External(
      hashedMessage,
      ed25519Kp.privateKey
    );

    const extraVerificationData = serializeExtraVerificationDataVec([
      {
        kind: ExtraVerificationDataKind.Ed25519Syscall,
        signature: ed25519Sig,
      },
    ]);

    const ed25519SignerKey = new PublicKey(ed25519Kp.publicKey);
    const allRemainingAccounts = [
      {
        pubkey: nativeSigner.publicKey,
        isSigner: true,
        isWritable: false,
      },
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
            numSigners: 2,
            payload: {
              __kind: "Transaction",
              fields: [compiledIxBytes],
            },
          },
          extraVerificationData: null,
        },
        programId
      );
    patchEvd(syncIx, extraVerificationData);

    const { blockhash } = await connection.getLatestBlockhash();
    const message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: blockhash,
      instructions: [syncIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([creator, nativeSigner]);

    // Should fail: nonce 1 was already consumed, expected nonce 2
    await sendAndExpectFailure(tx);
  });

  // ---------------------------------------------------------------
  // Two different external signers approve same proposal
  // Both nonces increment independently
  // ---------------------------------------------------------------
  it("two different external signers approve same proposal — both nonces increment", async () => {
    const { settingsPda, creator, nativeSigner, ed25519Keypairs } =
      await createAccountWithSigners({ numEd25519Signers: 2, threshold: 3 });

    await new Promise((resolve) => setTimeout(resolve, 1000));

    // Verify initial nonces are 0
    assert.strictEqual(await getEd25519SignerNonce(settingsPda, ed25519Keypairs[0].publicKey), 0n);
    assert.strictEqual(await getEd25519SignerNonce(settingsPda, ed25519Keypairs[1].publicKey), 0n);

    // Create proposal
    const settings = await Settings.fromAccountAddress(connection, settingsPda);
    const txIndex = BigInt(settings.transactionIndex.toString()) + 1n;

    let sig = await smartAccount.rpc.createSettingsTransactionV2({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex: txIndex,
      creator: nativeSigner.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 3 }],
      programId,
    });
    await connection.confirmTransaction(sig);

    sig = await smartAccount.rpc.createProposalV2({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex: txIndex,
      creator: nativeSigner,
      programId,
    });
    await connection.confirmTransaction(sig);

    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: txIndex,
      programId,
    });

    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.Ed25519Precompile,
    });

    // External signer #1 approves (nonce 0 → 1)
    const hashedMsg1 = buildVoteMessage(proposalPda, 0, txIndex, 1n);
    const edSig1 = signEd25519External(
      hashedMsg1,
      ed25519Keypairs[0].privateKey
    );
    const precompileIx1 = buildEd25519PrecompileInstruction(
      edSig1,
      ed25519Keypairs[0].publicKey,
      hashedMsg1
    );
    const signerKey1 = new PublicKey(ed25519Keypairs[0].publicKey);
    const approveIx1 =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey1,
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
    patchEvd(approveIx1, evdBytes);

    let message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx1, approveIx1],
    }).compileToV0Message();
    let tx = new VersionedTransaction(message);
    tx.sign([creator]);

    await sendAndConfirm(tx, "Signer #1 approves");

    // Verify signer #1 nonce = 1, signer #2 nonce still = 0
    assert.strictEqual(
      await getEd25519SignerNonce(settingsPda, ed25519Keypairs[0].publicKey),
      1n,
      "Signer #1 nonce should be 1"
    );
    assert.strictEqual(
      await getEd25519SignerNonce(settingsPda, ed25519Keypairs[1].publicKey),
      0n,
      "Signer #2 nonce should still be 0"
    );

    // External signer #2 approves (nonce 0 → 1)
    const hashedMsg2 = buildVoteMessage(proposalPda, 0, txIndex, 1n);
    const edSig2 = signEd25519External(
      hashedMsg2,
      ed25519Keypairs[1].privateKey
    );
    const precompileIx2 = buildEd25519PrecompileInstruction(
      edSig2,
      ed25519Keypairs[1].publicKey,
      hashedMsg2
    );
    const signerKey2 = new PublicKey(ed25519Keypairs[1].publicKey);
    const approveIx2 =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey2,
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
    patchEvd(approveIx2, evdBytes);

    message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx2, approveIx2],
    }).compileToV0Message();
    tx = new VersionedTransaction(message);
    tx.sign([creator]);

    await sendAndConfirm(tx, "Signer #2 approves");

    // Verify both nonces are now 1
    assert.strictEqual(
      await getEd25519SignerNonce(settingsPda, ed25519Keypairs[0].publicKey),
      1n,
      "Signer #1 nonce should still be 1"
    );
    assert.strictEqual(
      await getEd25519SignerNonce(settingsPda, ed25519Keypairs[1].publicKey),
      1n,
      "Signer #2 nonce should be 1"
    );
  });

  // ---------------------------------------------------------------
  // Payload mismatch — wrong payload hash in sync consensus message
  // ---------------------------------------------------------------
  it("payload mismatch — wrong payload hash in sync consensus message rejected", async () => {
    const { settingsPda, creator, nativeSigner, ed25519Keypairs } =
      await createAccountWithSigners({ numEd25519Signers: 1, threshold: 2 });
    const ed25519Kp = ed25519Keypairs[0];

    await new Promise((resolve) => setTimeout(resolve, 1000));

    // Fund the vault
    const [vaultPda] = smartAccount.getSmartAccountPda({
      settingsPda,
      accountIndex: 0,
      programId,
    });
    await connection.confirmTransaction(
      await connection.requestAirdrop(vaultPda, 5 * LAMPORTS_PER_SOL)
    );

    const settings = await Settings.fromAccountAddress(connection, settingsPda);
    const transactionIndex = BigInt(settings.transactionIndex.toString());

    const receiver = Keypair.generate();
    const transferIx = SystemProgram.transfer({
      fromPubkey: vaultPda,
      toPubkey: receiver.publicKey,
      lamports: LAMPORTS_PER_SOL,
    });

    const { instructions: rawCompiledIxBytes, accounts: txAccounts } =
      smartAccount.utils.instructionsToSynchronousTransactionDetailsV2({
        vaultPda,
        members: [],
        transaction_instructions: [transferIx],
      });
    const compiledIxBytes = Buffer.from(rawCompiledIxBytes);

    // Build a WRONG payload hash — external signer signed a different payload
    const wrongPayloadHash = sha256(Buffer.from("wrong payload data"));

    const hashedMessage = buildSyncConsensusMessage(
      settingsPda,
      transactionIndex,
      1n,
      wrongPayloadHash // Does not match actual instructions
    );

    const ed25519Sig = signEd25519External(
      hashedMessage,
      ed25519Kp.privateKey
    );
    const extraVerificationData = serializeExtraVerificationDataVec([
      {
        kind: ExtraVerificationDataKind.Ed25519Syscall,
        signature: ed25519Sig,
      },
    ]);

    const ed25519SignerKey = new PublicKey(ed25519Kp.publicKey);
    const allRemainingAccounts = [
      {
        pubkey: nativeSigner.publicKey,
        isSigner: true,
        isWritable: false,
      },
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
            numSigners: 2,
            payload: {
              __kind: "Transaction",
              fields: [compiledIxBytes],
            },
          },
          extraVerificationData: null,
        },
        programId
      );
    patchEvd(syncIx, extraVerificationData);

    const { blockhash } = await connection.getLatestBlockhash();
    const message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: blockhash,
      instructions: [syncIx],
    }).compileToV0Message();

    const tx = new VersionedTransaction(message);
    tx.sign([creator, nativeSigner]);

    // Should fail: external signer committed to wrong payload
    await sendAndExpectFailure(tx);
  });

  // ---------------------------------------------------------------
  // Cross-proposal nonce: nonce consumed in proposal A prevents
  // replay on proposal B (stale nonce), but correct nonce succeeds
  // ---------------------------------------------------------------
  it("cross-proposal nonce — stale nonce from proposal A rejected on proposal B", async () => {
    const { settingsPda, creator, nativeSigner, ed25519Keypairs } =
      await createAccountWithSigners({ numEd25519Signers: 1, threshold: 2 });
    const ed25519Kp = ed25519Keypairs[0];
    const signerKey = new PublicKey(ed25519Kp.publicKey);

    await new Promise((resolve) => setTimeout(resolve, 1000));

    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.Ed25519Precompile,
    });

    // --- Approve Proposal A (nonce 0 → 1) ---
    let settings = await Settings.fromAccountAddress(connection, settingsPda);
    const txIndexA = BigInt(settings.transactionIndex.toString()) + 1n;

    let sig = await smartAccount.rpc.createSettingsTransactionV2({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex: txIndexA,
      creator: nativeSigner.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
      programId,
    });
    await connection.confirmTransaction(sig);

    sig = await smartAccount.rpc.createProposalV2({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex: txIndexA,
      creator: nativeSigner,
      programId,
    });
    await connection.confirmTransaction(sig);

    const [proposalPdaA] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: txIndexA,
      programId,
    });

    const hashedMsgA = buildVoteMessage(proposalPdaA, 0, txIndexA, 1n);
    const edSigA = signEd25519External(hashedMsgA, ed25519Kp.privateKey);
    const precompileIxA = buildEd25519PrecompileInstruction(
      edSigA,
      ed25519Kp.publicKey,
      hashedMsgA
    );
    const approveIxA =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey,
          proposal: proposalPdaA,
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
    patchEvd(approveIxA, evdBytes);

    let message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIxA, approveIxA],
    }).compileToV0Message();
    let tx = new VersionedTransaction(message);
    tx.sign([creator]);

    await sendAndConfirm(tx, "Approve proposal A");
    assert.strictEqual(await getEd25519SignerNonce(settingsPda, ed25519Kp.publicKey), 1n);

    // --- Create Proposal B ---
    settings = await Settings.fromAccountAddress(connection, settingsPda);
    const txIndexB = BigInt(settings.transactionIndex.toString()) + 1n;

    sig = await smartAccount.rpc.createSettingsTransactionV2({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex: txIndexB,
      creator: nativeSigner.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
      programId,
    });
    await connection.confirmTransaction(sig);

    sig = await smartAccount.rpc.createProposalV2({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex: txIndexB,
      creator: nativeSigner,
      programId,
    });
    await connection.confirmTransaction(sig);

    const [proposalPdaB] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex: txIndexB,
      programId,
    });

    // --- Try to approve proposal B with STALE nonce=1 (should be 2) ---
    const hashedMsgBStale = buildVoteMessage(proposalPdaB, 0, txIndexB, 1n);
    const edSigBStale = signEd25519External(
      hashedMsgBStale,
      ed25519Kp.privateKey
    );
    const precompileIxBStale = buildEd25519PrecompileInstruction(
      edSigBStale,
      ed25519Kp.publicKey,
      hashedMsgBStale
    );
    const approveIxBStale =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey,
          proposal: proposalPdaB,
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
    patchEvd(approveIxBStale, evdBytes);

    message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIxBStale, approveIxBStale],
    }).compileToV0Message();
    tx = new VersionedTransaction(message);
    tx.sign([creator]);

    // Should fail: nonce 1 consumed by proposal A, expected nonce 2
    await sendAndExpectFailure(tx);

    // --- Now approve with correct nonce=2 ---
    const hashedMsgBCorrect = buildVoteMessage(proposalPdaB, 0, txIndexB, 2n);
    const edSigBCorrect = signEd25519External(
      hashedMsgBCorrect,
      ed25519Kp.privateKey
    );
    const precompileIxBCorrect = buildEd25519PrecompileInstruction(
      edSigBCorrect,
      ed25519Kp.publicKey,
      hashedMsgBCorrect
    );
    const approveIxBCorrect =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey,
          proposal: proposalPdaB,
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
    patchEvd(approveIxBCorrect, evdBytes);

    message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIxBCorrect, approveIxBCorrect],
    }).compileToV0Message();
    tx = new VersionedTransaction(message);
    tx.sign([creator]);

    await sendAndConfirm(tx, "Approve proposal B (correct nonce=2)");
    assert.strictEqual(
      await getEd25519SignerNonce(settingsPda, ed25519Kp.publicKey),
      2n,
      "Nonce should be 2 after approving proposal B"
    );
  });
});
