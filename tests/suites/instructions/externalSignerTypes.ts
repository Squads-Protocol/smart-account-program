import * as smartAccount from "@sqds/smart-account";
import {
  Keypair,
  PublicKey,
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
  generateSecp256k1Keypair,
  signSecp256k1,
  generateP256Keypair,
  signP256,
  buildEd25519PrecompileInstruction,
  buildSecp256k1PrecompileInstruction,
  buildSecp256r1PrecompileInstruction,
  base64urlEncode,
  buildVoteMessage,
} from "../../utils";
import {
  ExtraVerificationDataKind,
  serializeSingleExtraVerificationData,
} from "../../helpers/extraVerificationData";
import { sha256 } from "@noble/hashes/sha256";
import { keccak_256 } from "@noble/hashes/sha3";

const { Settings, Proposal } = smartAccount.accounts;
const { Permissions, Permission } = smartAccount.types;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

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

describe("Instructions / external_signer_types", () => {
  // ---------------------------------------------------------------
  // Account creation with each external signer type
  // ---------------------------------------------------------------

  it("create smart account with Ed25519External signer", async () => {
    const creator = await generateFundedKeypair(connection);
    const ed25519Keypair = generateEd25519ExternalKeypair();
    const treasury = getTestProgramTreasury();

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    const externalSigner: smartAccount.generated.SmartAccountSigner = {
      __kind: "Ed25519External",
      permissions: { mask: Permissions.all().mask },
      data: {
        externalPubkey: Array.from(ed25519Keypair.publicKey),
        sessionKeyData: { key: PublicKey.default, expiration: 0 },
      },
      nonce: 0,
    };

    const sig = await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [externalSigner],
      timeLock: 0,
      rentCollector: null,
      programId,
    });
    await connection.confirmTransaction(sig);

    const settings = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settings.signers.length, 1);

    const signer: any = settings.signers[0];
    assert.strictEqual(signer.__kind, "Ed25519External");
    assert.deepStrictEqual(
      Buffer.from(signer.data.externalPubkey),
      Buffer.from(ed25519Keypair.publicKey)
    );
    assert.strictEqual(Number(signer.nonce), 0);
  });

  it("create smart account with Secp256k1 signer", async () => {
    const creator = await generateFundedKeypair(connection);
    const secp256k1Keypair = generateSecp256k1Keypair();
    const treasury = getTestProgramTreasury();

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    const secp256k1Signer: smartAccount.generated.SmartAccountSigner = {
      __kind: "Secp256k1",
      permissions: { mask: Permissions.all().mask },
      data: {
        uncompressedPubkey: Array.from(
          secp256k1Keypair.publicKeyUncompressed
        ),
        ethAddress: Array.from(secp256k1Keypair.ethAddress),
        hasEthAddress: true,
        sessionKeyData: { key: PublicKey.default, expiration: 0 },
      },
      nonce: 0,
    };

    const sig = await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [secp256k1Signer],
      timeLock: 0,
      rentCollector: null,
      programId,
    });
    await connection.confirmTransaction(sig);

    const settings = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settings.signers.length, 1);

    const signer: any = settings.signers[0];
    assert.strictEqual(signer.__kind, "Secp256k1");
    assert.deepStrictEqual(
      Buffer.from(signer.data.uncompressedPubkey),
      Buffer.from(secp256k1Keypair.publicKeyUncompressed)
    );
    assert.deepStrictEqual(
      Buffer.from(signer.data.ethAddress),
      Buffer.from(secp256k1Keypair.ethAddress)
    );
    assert.strictEqual(Number(signer.nonce), 0);
  });

  it("create smart account with P256Webauthn signer", async () => {
    const creator = await generateFundedKeypair(connection);
    const p256Keypair = generateP256Keypair();
    const treasury = getTestProgramTreasury();

    const rpIdBytes = Buffer.from("example.com", "utf-8");
    const rpIdPadded = Buffer.alloc(32);
    rpIdBytes.copy(rpIdPadded);
    const rpIdHash = sha256(rpIdBytes);

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    const p256Signer: smartAccount.generated.SmartAccountSigner = {
      __kind: "P256Webauthn",
      permissions: { mask: Permissions.all().mask },
      data: {
        compressedPubkey: Array.from(p256Keypair.publicKeyCompressed),
        rpIdLen: rpIdBytes.length,
        rpId: Array.from(rpIdPadded),
        rpIdHash: Array.from(rpIdHash),
        counter: 0,
        sessionKeyData: { key: PublicKey.default, expiration: 0 },
      },
      nonce: 0,
    };

    const sig = await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [p256Signer],
      timeLock: 0,
      rentCollector: null,
      programId,
    });
    await connection.confirmTransaction(sig);

    const settings = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settings.signers.length, 1);

    const signer: any = settings.signers[0];
    assert.strictEqual(signer.__kind, "P256Webauthn");
    assert.deepStrictEqual(
      Buffer.from(signer.data.compressedPubkey),
      Buffer.from(p256Keypair.publicKeyCompressed)
    );
    assert.strictEqual(Number(signer.nonce), 0);
    assert.strictEqual(Number(signer.data.counter), 0);
  });

  it("create smart account with P256Native signer", async () => {
    const creator = await generateFundedKeypair(connection);
    const p256Keypair = generateP256Keypair();
    const treasury = getTestProgramTreasury();

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    const p256NativeSigner: smartAccount.generated.SmartAccountSigner = {
      __kind: "P256Native",
      permissions: { mask: Permissions.all().mask },
      data: {
        compressedPubkey: Array.from(p256Keypair.publicKeyCompressed),
        sessionKeyData: { key: PublicKey.default, expiration: 0 },
      },
      nonce: 0,
    };

    const sig = await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [p256NativeSigner],
      timeLock: 0,
      rentCollector: null,
      programId,
    });
    await connection.confirmTransaction(sig);

    const settings = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settings.signers.length, 1);

    const signer: any = settings.signers[0];
    assert.strictEqual(signer.__kind, "P256Native");
    assert.deepStrictEqual(
      Buffer.from(signer.data.compressedPubkey),
      Buffer.from(p256Keypair.publicKeyCompressed)
    );
    assert.strictEqual(Number(signer.nonce), 0);
  });

  // ---------------------------------------------------------------
  // Full approve + execute flow with each external signer type
  // ---------------------------------------------------------------

  it("approve + execute settings tx with Ed25519External via precompile", async () => {
    const creator = await generateFundedKeypair(connection);
    const nativeSigner = await generateFundedKeypair(connection);
    const ed25519Keypair = generateEd25519ExternalKeypair();
    const treasury = getTestProgramTreasury();

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    const nativeSignerObj: smartAccount.generated.SmartAccountSigner = {
      __kind: "Native",
      key: nativeSigner.publicKey,
      permissions: {
        mask: Permission.Initiate | Permission.Vote | Permission.Execute,
      },
    };

    const externalSigner: smartAccount.generated.SmartAccountSigner = {
      __kind: "Ed25519External",
      permissions: { mask: Permission.Vote | Permission.Execute },
      data: {
        externalPubkey: Array.from(ed25519Keypair.publicKey),
        sessionKeyData: { key: PublicKey.default, expiration: 0 },
      },
      nonce: 0,
    };

    await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [nativeSignerObj, externalSigner],
      timeLock: 0,
      rentCollector: null,
      programId,
    });

    await new Promise((resolve) => setTimeout(resolve, 1000));

    // Create settings tx + proposal with native signer
    const transactionIndex = 1n;
    let signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex,
      creator: nativeSigner.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex,
      creator: nativeSigner,
      programId,
    });
    await connection.confirmTransaction(signature);

    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });

    // Sign with Ed25519External via precompile
    const hashedMessage = buildVoteMessage(proposalPda, 0, transactionIndex, 1n);
    const sig = signEd25519External(hashedMessage, ed25519Keypair.privateKey);

    const precompileIx = buildEd25519PrecompileInstruction(
      sig,
      ed25519Keypair.publicKey,
      hashedMessage
    );

    const signerKey = new PublicKey(ed25519Keypair.publicKey);
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

    let message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx, approveIx],
    }).compileToV0Message();

    let tx = new VersionedTransaction(message);
    tx.sign([creator]);

    signature = await connection.sendTransaction(tx, { skipPreflight: true });
    await connection.confirmTransaction(signature);

    const txResult = await connection.getTransaction(signature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    if (txResult?.meta?.err) {
      console.error("Ed25519 precompile TX logs:", txResult.meta.logMessages);
      throw new Error(
        `Ed25519 precompile TX failed: ${JSON.stringify(txResult.meta.err)}`
      );
    }

    const proposal = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(proposal.approved.length > 0, "Proposal should have approvals");

    // Execute with native signer
    const executeIx = smartAccount.instructions.executeSettingsTransaction({
      settingsPda,
      transactionIndex,
      signer: nativeSigner.publicKey,
      rentPayer: nativeSigner.publicKey,
      programId,
    });

    message = new TransactionMessage({
      payerKey: nativeSigner.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [executeIx],
    }).compileToV0Message();

    tx = new VersionedTransaction(message);
    tx.sign([nativeSigner]);

    signature = await connection.sendTransaction(tx);
    await connection.confirmTransaction(signature);

    const settings = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settings.threshold, 2);
  });

  it("approve + execute settings tx with Secp256k1 via precompile", async () => {
    const creator = await generateFundedKeypair(connection);
    const nativeSigner = await generateFundedKeypair(connection);
    const secp256k1Keypair = generateSecp256k1Keypair();
    const treasury = getTestProgramTreasury();

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    const nativeSignerObj: smartAccount.generated.SmartAccountSigner = {
      __kind: "Native",
      key: nativeSigner.publicKey,
      permissions: {
        mask: Permission.Initiate | Permission.Vote | Permission.Execute,
      },
    };

    const secp256k1Signer: smartAccount.generated.SmartAccountSigner = {
      __kind: "Secp256k1",
      permissions: { mask: Permission.Vote | Permission.Execute },
      data: {
        uncompressedPubkey: Array.from(
          secp256k1Keypair.publicKeyUncompressed
        ),
        ethAddress: Array.from(secp256k1Keypair.ethAddress),
        hasEthAddress: true,
        sessionKeyData: { key: PublicKey.default, expiration: 0 },
      },
      nonce: 0,
    };

    await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [nativeSignerObj, secp256k1Signer],
      timeLock: 0,
      rentCollector: null,
      programId,
    });

    await new Promise((resolve) => setTimeout(resolve, 1000));

    const transactionIndex = 1n;
    let signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex,
      creator: nativeSigner.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex,
      creator: nativeSigner,
      programId,
    });
    await connection.confirmTransaction(signature);

    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });

    // Secp256k1 precompile: sign keccak256(sha256(message_data))
    const hashedMessage = buildVoteMessage(proposalPda, 0, transactionIndex, 1n);
    const keccakHash = keccak_256(hashedMessage);
    const { signature: sig, recoveryId } = signSecp256k1(
      keccakHash,
      secp256k1Keypair.privateKey
    );

    const precompileIx = buildSecp256k1PrecompileInstruction(
      sig,
      recoveryId,
      hashedMessage,
      secp256k1Keypair.ethAddress,
      0
    );

    const signerKey = new PublicKey(
      secp256k1Keypair.publicKeyUncompressed.slice(0, 32)
    );
    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.Secp256k1Precompile,
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

    let message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx, approveIx],
    }).compileToV0Message();

    let tx = new VersionedTransaction(message);
    tx.sign([creator]);

    signature = await connection.sendTransaction(tx, { skipPreflight: true });
    await connection.confirmTransaction(signature);

    const txResult = await connection.getTransaction(signature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    if (txResult?.meta?.err) {
      console.error("Secp256k1 precompile TX logs:", txResult.meta.logMessages);
      throw new Error(
        `Secp256k1 precompile TX failed: ${JSON.stringify(txResult.meta.err)}`
      );
    }

    const proposal = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(proposal.approved.length > 0, "Proposal should have approvals");

    // Execute
    const executeIx = smartAccount.instructions.executeSettingsTransaction({
      settingsPda,
      transactionIndex,
      signer: nativeSigner.publicKey,
      rentPayer: nativeSigner.publicKey,
      programId,
    });

    message = new TransactionMessage({
      payerKey: nativeSigner.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [executeIx],
    }).compileToV0Message();

    tx = new VersionedTransaction(message);
    tx.sign([nativeSigner]);

    signature = await connection.sendTransaction(tx);
    await connection.confirmTransaction(signature);

    const settings = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settings.threshold, 2);
  });

  it("approve + execute settings tx with P256Webauthn via precompile", async () => {
    const creator = await generateFundedKeypair(connection);
    const nativeSigner = await generateFundedKeypair(connection);
    const p256Keypair = generateP256Keypair();
    const treasury = getTestProgramTreasury();

    const rpId = "example.com";
    const rpIdBytes = Buffer.from(rpId, "utf-8");
    const rpIdPadded = Buffer.alloc(32);
    rpIdBytes.copy(rpIdPadded);
    const rpIdHash = sha256(rpIdBytes);

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    const nativeSignerObj: smartAccount.generated.SmartAccountSigner = {
      __kind: "Native",
      key: nativeSigner.publicKey,
      permissions: {
        mask: Permission.Initiate | Permission.Vote | Permission.Execute,
      },
    };

    const p256Signer: smartAccount.generated.SmartAccountSigner = {
      __kind: "P256Webauthn",
      permissions: { mask: Permission.Vote | Permission.Execute },
      data: {
        compressedPubkey: Array.from(p256Keypair.publicKeyCompressed),
        rpIdLen: rpIdBytes.length,
        rpId: Array.from(rpIdPadded),
        rpIdHash: Array.from(rpIdHash),
        counter: 0,
        sessionKeyData: { key: PublicKey.default, expiration: 0 },
      },
      nonce: 0,
    };

    await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [nativeSignerObj, p256Signer],
      timeLock: 0,
      rentCollector: null,
      programId,
    });

    await new Promise((resolve) => setTimeout(resolve, 1000));

    const transactionIndex = 1n;
    let signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex,
      creator: nativeSigner.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex,
      creator: nativeSigner,
      programId,
    });
    await connection.confirmTransaction(signature);

    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });

    // Build the vote message hash (challenge)
    const hashedMessage = buildVoteMessage(proposalPda, 0, transactionIndex, 1n);

    // Build WebAuthn authenticator data + clientDataJSON
    const signCounter = 1; // Must be > stored counter (0)
    const counterBuf = Buffer.alloc(4);
    counterBuf.writeUInt32BE(signCounter);
    const authenticatorData = Buffer.concat([
      Buffer.from(rpIdHash),
      Buffer.from([0x01]), // flags: UP
      counterBuf,
    ]);

    const challengeB64url = base64urlEncode(hashedMessage);
    const clientDataJSON = `{"type":"webauthn.get","challenge":"${challengeB64url}","origin":"https://${rpId}","crossOrigin":false}`;
    const clientDataHash = sha256(Buffer.from(clientDataJSON, "utf-8"));

    // Precompile message = authenticatorData || clientDataHash
    const precompileMessage = Buffer.concat([
      authenticatorData,
      Buffer.from(clientDataHash),
    ]);

    const p256Sig = signP256(precompileMessage, p256Keypair.privateKey);

    const precompileIx = buildSecp256r1PrecompileInstruction(
      p256Sig,
      p256Keypair.publicKeyCompressed,
      precompileMessage
    );

    // P256 signer key: compressed_pubkey[0..32]
    const signerKey = new PublicKey(
      p256Keypair.publicKeyCompressed.slice(0, 32)
    );

    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.P256WebauthnPrecompile,
      typeAndFlags: 0x10, // TYPE_GET
      port: 0,
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

    let message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx, approveIx],
    }).compileToV0Message();

    let tx = new VersionedTransaction(message);
    tx.sign([creator]);

    signature = await connection.sendTransaction(tx, { skipPreflight: true });
    await connection.confirmTransaction(signature);

    const txResult = await connection.getTransaction(signature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    if (txResult?.meta?.err) {
      console.error("P256 precompile TX logs:", txResult.meta.logMessages);
      throw new Error(
        `P256 precompile TX failed: ${JSON.stringify(txResult.meta.err)}`
      );
    }

    const proposal = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(proposal.approved.length > 0, "Proposal should have approvals");

    // Execute
    const executeIx = smartAccount.instructions.executeSettingsTransaction({
      settingsPda,
      transactionIndex,
      signer: nativeSigner.publicKey,
      rentPayer: nativeSigner.publicKey,
      programId,
    });

    message = new TransactionMessage({
      payerKey: nativeSigner.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [executeIx],
    }).compileToV0Message();

    tx = new VersionedTransaction(message);
    tx.sign([nativeSigner]);

    signature = await connection.sendTransaction(tx);
    await connection.confirmTransaction(signature);

    const settings = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settings.threshold, 2);
  });

  it("approve + execute settings tx with P256Native via precompile", async () => {
    const creator = await generateFundedKeypair(connection);
    const nativeSigner = await generateFundedKeypair(connection);
    const p256Keypair = generateP256Keypair();
    const treasury = getTestProgramTreasury();

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    const nativeSignerObj: smartAccount.generated.SmartAccountSigner = {
      __kind: "Native",
      key: nativeSigner.publicKey,
      permissions: {
        mask: Permission.Initiate | Permission.Vote | Permission.Execute,
      },
    };

    const p256NativeSigner: smartAccount.generated.SmartAccountSigner = {
      __kind: "P256Native",
      permissions: { mask: Permission.Vote | Permission.Execute },
      data: {
        compressedPubkey: Array.from(p256Keypair.publicKeyCompressed),
        sessionKeyData: { key: PublicKey.default, expiration: 0 },
      },
      nonce: 0,
    };

    await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [nativeSignerObj, p256NativeSigner],
      timeLock: 0,
      rentCollector: null,
      programId,
    });

    await new Promise((resolve) => setTimeout(resolve, 1000));

    const transactionIndex = 1n;
    let signature = await smartAccount.rpc.createSettingsTransaction({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex,
      creator: nativeSigner.publicKey,
      actions: [{ __kind: "ChangeThreshold", newThreshold: 2 }],
      programId,
    });
    await connection.confirmTransaction(signature);

    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex,
      creator: nativeSigner,
      programId,
    });
    await connection.confirmTransaction(signature);

    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });

    // P256Native: sign the raw message hash directly (no WebAuthn wrapping)
    const hashedMessage = buildVoteMessage(proposalPda, 0, transactionIndex, 1n);
    const p256Sig = signP256(hashedMessage, p256Keypair.privateKey);

    const precompileIx = buildSecp256r1PrecompileInstruction(
      p256Sig,
      p256Keypair.publicKeyCompressed,
      hashedMessage
    );

    // P256 signer key: compressed_pubkey[0..32]
    const signerKey = new PublicKey(
      p256Keypair.publicKeyCompressed.slice(0, 32)
    );

    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.P256NativePrecompile,
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

    let message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx, approveIx],
    }).compileToV0Message();

    let tx = new VersionedTransaction(message);
    tx.sign([creator]);

    signature = await connection.sendTransaction(tx, { skipPreflight: true });
    await connection.confirmTransaction(signature);

    const txResult = await connection.getTransaction(signature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    if (txResult?.meta?.err) {
      console.error("P256Native precompile TX logs:", txResult.meta.logMessages);
      throw new Error(
        `P256Native precompile TX failed: ${JSON.stringify(txResult.meta.err)}`
      );
    }

    const proposal = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(proposal.approved.length > 0, "Proposal should have approvals");

    // Execute with native signer
    const executeIx = smartAccount.instructions.executeSettingsTransaction({
      settingsPda,
      transactionIndex,
      signer: nativeSigner.publicKey,
      rentPayer: nativeSigner.publicKey,
      programId,
    });

    message = new TransactionMessage({
      payerKey: nativeSigner.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [executeIx],
    }).compileToV0Message();

    tx = new VersionedTransaction(message);
    tx.sign([nativeSigner]);

    signature = await connection.sendTransaction(tx);
    await connection.confirmTransaction(signature);

    const settings = await Settings.fromAccountAddress(
      connection,
      settingsPda
    );
    assert.strictEqual(settings.threshold, 2);
  });
});
