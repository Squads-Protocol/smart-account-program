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
  buildEd25519PrecompileInstruction,
  generateSecp256k1Keypair,
  signSecp256k1,
  buildSecp256k1PrecompileInstruction,
  generateP256Keypair,
  signP256,
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
const { Permission, Permissions } = smartAccount.types;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

// ---------------------------------------------------------------------------
// Message builders (match on-chain Hasher patterns)
// ---------------------------------------------------------------------------

function buildSessionKeyCreateMessage(
  settingsPda: PublicKey,
  signerKey: PublicKey,
  sessionKey: PublicKey,
  sessionKeyExpiration: bigint,
  nextNonce: bigint
): Uint8Array {
  const expBytes = Buffer.alloc(8);
  expBytes.writeBigUInt64LE(sessionKeyExpiration);
  const nonceBytes = Buffer.alloc(8);
  nonceBytes.writeBigUInt64LE(nextNonce);
  return sha256(
    Buffer.concat([
      Buffer.from("create_session_key_v2"),
      settingsPda.toBuffer(),
      signerKey.toBuffer(),
      sessionKey.toBuffer(),
      expBytes,
      nonceBytes,
    ])
  );
}

function buildSessionKeyRevokeMessage(
  settingsPda: PublicKey,
  signerKey: PublicKey,
  nextNonce: bigint
): Uint8Array {
  const nonceBytes = Buffer.alloc(8);
  nonceBytes.writeBigUInt64LE(nextNonce);
  return sha256(
    Buffer.concat([
      Buffer.from("revoke_session_key_v2"),
      settingsPda.toBuffer(),
      signerKey.toBuffer(),
      nonceBytes,
    ])
  );
}

/** Patch EVD in instruction data */
function patchEvd(ix: { data: Buffer }, evdBytes: Uint8Array): void {
  const w = ix.data.subarray(0, ix.data.length - 1);
  ix.data = Buffer.concat([w, Buffer.from([0x01]), Buffer.from(evdBytes)]);
}

/** Send tx, confirm, and assert on-chain success */
async function sendAndConfirm(
  payer: Keypair,
  ixs: any[],
  extraSigners: Keypair[] = []
): Promise<string> {
  const { blockhash, lastValidBlockHeight } =
    await connection.getLatestBlockhash();
  const msg = new TransactionMessage({
    payerKey: payer.publicKey,
    recentBlockhash: blockhash,
    instructions: ixs,
  }).compileToV0Message();
  const tx = new VersionedTransaction(msg);
  tx.sign([payer, ...extraSigners]);

  const sig = await connection.sendTransaction(tx, { skipPreflight: true });
  await connection.confirmTransaction({ signature: sig, blockhash, lastValidBlockHeight });

  const result = await connection.getTransaction(sig, {
    commitment: "confirmed",
    maxSupportedTransactionVersion: 0,
  });
  if (result?.meta?.err) {
    console.error("TX logs:", result.meta.logMessages);
    throw new Error(`TX failed: ${JSON.stringify(result.meta.err)}`);
  }
  return sig;
}

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

describe("Instructions / session_keys", () => {
  // =======================================================================
  // Ed25519External: create + revoke + use
  // =======================================================================

  describe("Ed25519External", () => {
    let nativeSigner: Keypair;
    let ed25519Keypair: ReturnType<typeof generateEd25519ExternalKeypair>;
    let settingsPda: PublicKey;
    let signerKeyId: PublicKey;

    before(async () => {
      const creator = await generateFundedKeypair(connection);
      nativeSigner = await generateFundedKeypair(connection);
      ed25519Keypair = generateEd25519ExternalKeypair();
      const treasury = getTestProgramTreasury();
      const accountIndex = await getNextAccountIndex(connection, programId);
      [settingsPda] = smartAccount.getSettingsPda({ accountIndex, programId });
      signerKeyId = new PublicKey(ed25519Keypair.publicKey);

      await smartAccount.rpc.createSmartAccount({
        connection,
        treasury,
        creator,
        settings: settingsPda,
        settingsAuthority: null,
        threshold: 1,
        signers: [
          {
            __kind: "Native" as const,
            key: nativeSigner.publicKey,
            permissions: { mask: Permission.Initiate | Permission.Vote | Permission.Execute },
          },
          {
            __kind: "Ed25519External" as const,
            permissions: { mask: Permission.Initiate | Permission.Vote | Permission.Execute },
            data: {
              externalPubkey: Array.from(ed25519Keypair.publicKey),
              sessionKeyData: { key: PublicKey.default, expiration: 0 },
            },
            nonce: 0,
          },
        ],
        timeLock: 0,
        rentCollector: null,
        programId,
      });
      await new Promise((r) => setTimeout(r, 1000));
    });

    it("create session key via precompile", async () => {
      const sessionKeypair = Keypair.generate();
      const exp = BigInt(Math.floor(Date.now() / 1000) + 3600);

      const msg = buildSessionKeyCreateMessage(settingsPda, signerKeyId, sessionKeypair.publicKey, exp, 1n);
      const sig = signEd25519External(msg, ed25519Keypair.privateKey);
      const precompileIx = buildEd25519PrecompileInstruction(sig, ed25519Keypair.publicKey, msg);

      const evd = serializeSingleExtraVerificationData({ kind: ExtraVerificationDataKind.Ed25519Precompile });
      const ix = smartAccount.instructions.createSessionKey({
        settingsPda,
        signer: signerKeyId,
        args: { sessionKey: sessionKeypair.publicKey, sessionKeyExpiration: exp },
        extraVerificationData: evd,
        programId,
      });
      ix.keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });

      await sendAndConfirm(nativeSigner, [precompileIx, ix]);

      const settings = await Settings.fromAccountAddress(connection, settingsPda);
      const s: any = (settings.signers as any[]).find((s: any) => s.__kind === "Ed25519External");
      assert.ok(s.data.sessionKeyData.key.equals(sessionKeypair.publicKey));
      assert.strictEqual(Number(s.data.sessionKeyData.expiration), Number(exp));
      assert.strictEqual(Number(s.nonce), 1);
    });

    it("revoke session key via precompile (nonce 1 -> 2)", async () => {
      const msg = buildSessionKeyRevokeMessage(settingsPda, signerKeyId, 2n);
      const sig = signEd25519External(msg, ed25519Keypair.privateKey);
      const precompileIx = buildEd25519PrecompileInstruction(sig, ed25519Keypair.publicKey, msg);

      const evd = serializeSingleExtraVerificationData({ kind: ExtraVerificationDataKind.Ed25519Precompile });
      const ix = smartAccount.instructions.revokeSessionKey({
        settingsPda,
        authority: signerKeyId,
        signerKey: signerKeyId,
        extraVerificationData: evd,
        programId,
      });
      ix.keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });

      await sendAndConfirm(nativeSigner, [precompileIx, ix]);

      const settings = await Settings.fromAccountAddress(connection, settingsPda);
      const s: any = (settings.signers as any[]).find((s: any) => s.__kind === "Ed25519External");
      assert.ok(s.data.sessionKeyData.key.equals(PublicKey.default));
      assert.strictEqual(Number(s.nonce), 2);
    });
  });

  // =======================================================================
  // Secp256k1: create + revoke
  // =======================================================================

  describe("Secp256k1", () => {
    let nativeSigner: Keypair;
    let secp256k1Keypair: ReturnType<typeof generateSecp256k1Keypair>;
    let settingsPda: PublicKey;
    let signerKeyId: PublicKey;

    before(async () => {
      const creator = await generateFundedKeypair(connection);
      nativeSigner = await generateFundedKeypair(connection);
      secp256k1Keypair = generateSecp256k1Keypair();
      const treasury = getTestProgramTreasury();
      const accountIndex = await getNextAccountIndex(connection, programId);
      [settingsPda] = smartAccount.getSettingsPda({ accountIndex, programId });
      signerKeyId = new PublicKey(secp256k1Keypair.publicKeyUncompressed.slice(0, 32));

      await smartAccount.rpc.createSmartAccount({
        connection,
        treasury,
        creator,
        settings: settingsPda,
        settingsAuthority: null,
        threshold: 1,
        signers: [
          {
            __kind: "Native" as const,
            key: nativeSigner.publicKey,
            permissions: { mask: Permission.Initiate | Permission.Vote | Permission.Execute },
          },
          {
            __kind: "Secp256k1" as const,
            permissions: { mask: Permission.Initiate | Permission.Vote | Permission.Execute },
            data: {
              uncompressedPubkey: Array.from(secp256k1Keypair.publicKeyUncompressed),
              ethAddress: Array.from(secp256k1Keypair.ethAddress),
              hasEthAddress: true,
              sessionKeyData: { key: PublicKey.default, expiration: 0 },
            },
            nonce: 0,
          },
        ],
        timeLock: 0,
        rentCollector: null,
        programId,
      });
      await new Promise((r) => setTimeout(r, 1000));
    });

    function signForSecp256k1(hashedMessage: Uint8Array) {
      const keccakHash = keccak_256(hashedMessage);
      const { signature: sig, recoveryId } = signSecp256k1(keccakHash, secp256k1Keypair.privateKey);
      return buildSecp256k1PrecompileInstruction(sig, recoveryId, hashedMessage, secp256k1Keypair.ethAddress, 0);
    }

    it("create session key via precompile", async () => {
      const sessionKeypair = Keypair.generate();
      const exp = BigInt(Math.floor(Date.now() / 1000) + 3600);

      const msg = buildSessionKeyCreateMessage(settingsPda, signerKeyId, sessionKeypair.publicKey, exp, 1n);
      const precompileIx = signForSecp256k1(msg);

      const evd = serializeSingleExtraVerificationData({ kind: ExtraVerificationDataKind.Secp256k1Precompile });
      const ix = smartAccount.instructions.createSessionKey({
        settingsPda,
        signer: signerKeyId,
        args: { sessionKey: sessionKeypair.publicKey, sessionKeyExpiration: exp },
        extraVerificationData: evd,
        programId,
      });
      ix.keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });

      await sendAndConfirm(nativeSigner, [precompileIx, ix]);

      const settings = await Settings.fromAccountAddress(connection, settingsPda);
      const s: any = (settings.signers as any[]).find((s: any) => s.__kind === "Secp256k1");
      assert.ok(s.data.sessionKeyData.key.equals(sessionKeypair.publicKey));
      assert.strictEqual(Number(s.nonce), 1);
    });

    it("revoke session key via precompile (nonce 1 -> 2)", async () => {
      const msg = buildSessionKeyRevokeMessage(settingsPda, signerKeyId, 2n);
      const precompileIx = signForSecp256k1(msg);

      const evd = serializeSingleExtraVerificationData({ kind: ExtraVerificationDataKind.Secp256k1Precompile });
      const ix = smartAccount.instructions.revokeSessionKey({
        settingsPda,
        authority: signerKeyId,
        signerKey: signerKeyId,
        extraVerificationData: evd,
        programId,
      });
      ix.keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });

      await sendAndConfirm(nativeSigner, [precompileIx, ix]);

      const settings = await Settings.fromAccountAddress(connection, settingsPda);
      const s: any = (settings.signers as any[]).find((s: any) => s.__kind === "Secp256k1");
      assert.ok(s.data.sessionKeyData.key.equals(PublicKey.default));
      assert.strictEqual(Number(s.nonce), 2);
    });
  });

  // =======================================================================
  // P256Webauthn: create + revoke
  // =======================================================================

  describe("P256Webauthn", () => {
    let nativeSigner: Keypair;
    let p256Keypair: ReturnType<typeof generateP256Keypair>;
    let settingsPda: PublicKey;
    let signerKeyId: PublicKey;
    const rpId = "example.com";
    const rpIdBytes = Buffer.from(rpId, "utf-8");
    const rpIdPadded = Buffer.alloc(32);
    let rpIdHash: Uint8Array;
    let webauthnCounter: number;

    before(async () => {
      rpIdBytes.copy(rpIdPadded);
      rpIdHash = sha256(rpIdBytes);
      webauthnCounter = 0;

      const creator = await generateFundedKeypair(connection);
      nativeSigner = await generateFundedKeypair(connection);
      p256Keypair = generateP256Keypair();
      const treasury = getTestProgramTreasury();
      const accountIndex = await getNextAccountIndex(connection, programId);
      [settingsPda] = smartAccount.getSettingsPda({ accountIndex, programId });
      signerKeyId = new PublicKey(p256Keypair.publicKeyCompressed.slice(0, 32));

      await smartAccount.rpc.createSmartAccount({
        connection,
        treasury,
        creator,
        settings: settingsPda,
        settingsAuthority: null,
        threshold: 1,
        signers: [
          {
            __kind: "Native" as const,
            key: nativeSigner.publicKey,
            permissions: { mask: Permission.Initiate | Permission.Vote | Permission.Execute },
          },
          {
            __kind: "P256Webauthn" as const,
            permissions: { mask: Permission.Initiate | Permission.Vote | Permission.Execute },
            data: {
              compressedPubkey: Array.from(p256Keypair.publicKeyCompressed),
              rpIdLen: rpIdBytes.length,
              rpId: Array.from(rpIdPadded),
              rpIdHash: Array.from(rpIdHash),
              counter: 0,
              sessionKeyData: { key: PublicKey.default, expiration: 0 },
            },
            nonce: 0,
          },
        ],
        timeLock: 0,
        rentCollector: null,
        programId,
      });
      await new Promise((r) => setTimeout(r, 1000));
    });

    function signForP256(hashedMessage: Uint8Array) {
      webauthnCounter++;
      const counterBuf = Buffer.alloc(4);
      counterBuf.writeUInt32BE(webauthnCounter);
      const authenticatorData = Buffer.concat([
        Buffer.from(rpIdHash),
        Buffer.from([0x01]),
        counterBuf,
      ]);
      const challengeB64url = base64urlEncode(hashedMessage);
      const clientDataJSON = `{"type":"webauthn.get","challenge":"${challengeB64url}","origin":"https://${rpId}","crossOrigin":false}`;
      const clientDataHash = sha256(Buffer.from(clientDataJSON, "utf-8"));
      const precompileMessage = Buffer.concat([authenticatorData, Buffer.from(clientDataHash)]);
      const sig = signP256(precompileMessage, p256Keypair.privateKey);
      return buildSecp256r1PrecompileInstruction(sig, p256Keypair.publicKeyCompressed, precompileMessage);
    }

    it("create session key via precompile", async () => {
      const sessionKeypair = Keypair.generate();
      const exp = BigInt(Math.floor(Date.now() / 1000) + 3600);

      const msg = buildSessionKeyCreateMessage(settingsPda, signerKeyId, sessionKeypair.publicKey, exp, 1n);
      const precompileIx = signForP256(msg);

      const evd = serializeSingleExtraVerificationData({
        kind: ExtraVerificationDataKind.P256WebauthnPrecompile,
        typeAndFlags: 0x10,
        port: 0,
      });
      const ix = smartAccount.instructions.createSessionKey({
        settingsPda,
        signer: signerKeyId,
        args: { sessionKey: sessionKeypair.publicKey, sessionKeyExpiration: exp },
        extraVerificationData: evd,
        programId,
      });
      ix.keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });

      await sendAndConfirm(nativeSigner, [precompileIx, ix]);

      const settings = await Settings.fromAccountAddress(connection, settingsPda);
      const s: any = (settings.signers as any[]).find((s: any) => s.__kind === "P256Webauthn");
      assert.ok(s.data.sessionKeyData.key.equals(sessionKeypair.publicKey));
      assert.strictEqual(Number(s.nonce), 1);
    });

    it("revoke session key via precompile (nonce 1 -> 2)", async () => {
      const msg = buildSessionKeyRevokeMessage(settingsPda, signerKeyId, 2n);
      const precompileIx = signForP256(msg);

      const evd = serializeSingleExtraVerificationData({
        kind: ExtraVerificationDataKind.P256WebauthnPrecompile,
        typeAndFlags: 0x10,
        port: 0,
      });
      const ix = smartAccount.instructions.revokeSessionKey({
        settingsPda,
        authority: signerKeyId,
        signerKey: signerKeyId,
        extraVerificationData: evd,
        programId,
      });
      ix.keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });

      await sendAndConfirm(nativeSigner, [precompileIx, ix]);

      const settings = await Settings.fromAccountAddress(connection, settingsPda);
      const s: any = (settings.signers as any[]).find((s: any) => s.__kind === "P256Webauthn");
      assert.ok(s.data.sessionKeyData.key.equals(PublicKey.default));
      assert.strictEqual(Number(s.nonce), 2);
    });
  });

  // =======================================================================
  // P256Native: create + revoke
  // =======================================================================

  describe("P256Native", () => {
    let nativeSigner: Keypair;
    let p256Keypair: ReturnType<typeof generateP256Keypair>;
    let settingsPda: PublicKey;
    let signerKeyId: PublicKey;

    before(async () => {
      const creator = await generateFundedKeypair(connection);
      nativeSigner = await generateFundedKeypair(connection);
      p256Keypair = generateP256Keypair();
      const treasury = getTestProgramTreasury();
      const accountIndex = await getNextAccountIndex(connection, programId);
      [settingsPda] = smartAccount.getSettingsPda({ accountIndex, programId });
      signerKeyId = new PublicKey(p256Keypair.publicKeyCompressed.slice(0, 32));

      await smartAccount.rpc.createSmartAccount({
        connection,
        treasury,
        creator,
        settings: settingsPda,
        settingsAuthority: null,
        threshold: 1,
        signers: [
          {
            __kind: "Native" as const,
            key: nativeSigner.publicKey,
            permissions: { mask: Permission.Initiate | Permission.Vote | Permission.Execute },
          },
          {
            __kind: "P256Native" as const,
            permissions: { mask: Permission.Initiate | Permission.Vote | Permission.Execute },
            data: {
              compressedPubkey: Array.from(p256Keypair.publicKeyCompressed),
              sessionKeyData: { key: PublicKey.default, expiration: 0 },
            },
            nonce: 0,
          },
        ],
        timeLock: 0,
        rentCollector: null,
        programId,
      });
      await new Promise((r) => setTimeout(r, 1000));
    });

    function signForP256Native(hashedMessage: Uint8Array) {
      // P256Native: sign the raw message hash directly (no WebAuthn wrapping)
      const sig = signP256(hashedMessage, p256Keypair.privateKey);
      return buildSecp256r1PrecompileInstruction(sig, p256Keypair.publicKeyCompressed, hashedMessage);
    }

    it("create session key via precompile", async () => {
      const sessionKeypair = Keypair.generate();
      const exp = BigInt(Math.floor(Date.now() / 1000) + 3600);

      const msg = buildSessionKeyCreateMessage(settingsPda, signerKeyId, sessionKeypair.publicKey, exp, 1n);
      const precompileIx = signForP256Native(msg);

      const evd = serializeSingleExtraVerificationData({ kind: ExtraVerificationDataKind.P256NativePrecompile });
      const ix = smartAccount.instructions.createSessionKey({
        settingsPda,
        signer: signerKeyId,
        args: { sessionKey: sessionKeypair.publicKey, sessionKeyExpiration: exp },
        extraVerificationData: evd,
        programId,
      });
      ix.keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });

      await sendAndConfirm(nativeSigner, [precompileIx, ix]);

      const settings = await Settings.fromAccountAddress(connection, settingsPda);
      const s: any = (settings.signers as any[]).find((s: any) => s.__kind === "P256Native");
      assert.ok(s.data.sessionKeyData.key.equals(sessionKeypair.publicKey));
      assert.strictEqual(Number(s.nonce), 1);
    });

    it("revoke session key via precompile (nonce 1 -> 2)", async () => {
      const msg = buildSessionKeyRevokeMessage(settingsPda, signerKeyId, 2n);
      const precompileIx = signForP256Native(msg);

      const evd = serializeSingleExtraVerificationData({ kind: ExtraVerificationDataKind.P256NativePrecompile });
      const ix = smartAccount.instructions.revokeSessionKey({
        settingsPda,
        authority: signerKeyId,
        signerKey: signerKeyId,
        extraVerificationData: evd,
        programId,
      });
      ix.keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });

      await sendAndConfirm(nativeSigner, [precompileIx, ix]);

      const settings = await Settings.fromAccountAddress(connection, settingsPda);
      const s: any = (settings.signers as any[]).find((s: any) => s.__kind === "P256Native");
      assert.ok(s.data.sessionKeyData.key.equals(PublicKey.default));
      assert.strictEqual(Number(s.nonce), 2);
    });
  });

  // =======================================================================
  // Session key holder self-revoke
  // =======================================================================

  it("session key holder self-revokes", async () => {
    const creator = await generateFundedKeypair(connection);
    const nativeSigner = await generateFundedKeypair(connection);
    const ed25519Keypair = generateEd25519ExternalKeypair();
    const treasury = getTestProgramTreasury();
    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({ accountIndex, programId });
    const signerKeyId = new PublicKey(ed25519Keypair.publicKey);

    await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [
        {
          __kind: "Native" as const,
          key: nativeSigner.publicKey,
          permissions: { mask: Permissions.all().mask },
        },
        {
          __kind: "Ed25519External" as const,
          permissions: { mask: Permissions.all().mask },
          data: {
            externalPubkey: Array.from(ed25519Keypair.publicKey),
            sessionKeyData: { key: PublicKey.default, expiration: 0 },
          },
          nonce: 0,
        },
      ],
      timeLock: 0,
      rentCollector: null,
      programId,
    });
    await new Promise((r) => setTimeout(r, 1000));

    const sessionKeypair = Keypair.generate();
    const fundTx = await connection.requestAirdrop(sessionKeypair.publicKey, 2e9);
    await connection.confirmTransaction(fundTx);

    const exp = BigInt(Math.floor(Date.now() / 1000) + 3600);

    // Create session key
    const createMsg = buildSessionKeyCreateMessage(settingsPda, signerKeyId, sessionKeypair.publicKey, exp, 1n);
    const createSig = signEd25519External(createMsg, ed25519Keypair.privateKey);
    const createPrecompileIx = buildEd25519PrecompileInstruction(createSig, ed25519Keypair.publicKey, createMsg);

    const createEvd = serializeSingleExtraVerificationData({ kind: ExtraVerificationDataKind.Ed25519Precompile });
    const createIx = smartAccount.instructions.createSessionKey({
      settingsPda,
      signer: signerKeyId,
      args: { sessionKey: sessionKeypair.publicKey, sessionKeyExpiration: exp },
      extraVerificationData: createEvd,
      programId,
    });
    createIx.keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });

    await sendAndConfirm(nativeSigner, [createPrecompileIx, createIx]);

    // Session key holder self-revokes (no precompile, no EVD)
    const revokeIx = smartAccount.instructions.revokeSessionKey({
      settingsPda,
      authority: sessionKeypair.publicKey,
      signerKey: signerKeyId,
      programId,
    });

    await sendAndConfirm(nativeSigner, [revokeIx], [sessionKeypair]);

    const settings = await Settings.fromAccountAddress(connection, settingsPda);
    const s: any = (settings.signers as any[]).find((s: any) => s.__kind === "Ed25519External");
    assert.ok(s.data.sessionKeyData.key.equals(PublicKey.default));
  });

  // =======================================================================
  // Session key used to approve a proposal (on behalf of external signer)
  // =======================================================================

  it("session key approves proposal on behalf of Ed25519External signer", async () => {
    const creator = await generateFundedKeypair(connection);
    const nativeSigner = await generateFundedKeypair(connection);
    const ed25519Keypair = generateEd25519ExternalKeypair();
    const treasury = getTestProgramTreasury();
    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({ accountIndex, programId });
    const signerKeyId = new PublicKey(ed25519Keypair.publicKey);

    await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [
        {
          __kind: "Native" as const,
          key: nativeSigner.publicKey,
          permissions: { mask: Permission.Initiate | Permission.Vote | Permission.Execute },
        },
        {
          __kind: "Ed25519External" as const,
          permissions: { mask: Permission.Initiate | Permission.Vote | Permission.Execute },
          data: {
            externalPubkey: Array.from(ed25519Keypair.publicKey),
            sessionKeyData: { key: PublicKey.default, expiration: 0 },
          },
          nonce: 0,
        },
      ],
      timeLock: 0,
      rentCollector: null,
      programId,
    });
    await new Promise((r) => setTimeout(r, 1000));

    // Create session key
    const sessionKeypair = Keypair.generate();
    const fundTx = await connection.requestAirdrop(sessionKeypair.publicKey, 2e9);
    await connection.confirmTransaction(fundTx);

    const exp = BigInt(Math.floor(Date.now() / 1000) + 3600);
    const createMsg = buildSessionKeyCreateMessage(settingsPda, signerKeyId, sessionKeypair.publicKey, exp, 1n);
    const createSig = signEd25519External(createMsg, ed25519Keypair.privateKey);
    const createPrecompileIx = buildEd25519PrecompileInstruction(createSig, ed25519Keypair.publicKey, createMsg);

    const createEvd = serializeSingleExtraVerificationData({ kind: ExtraVerificationDataKind.Ed25519Precompile });
    const createIx = smartAccount.instructions.createSessionKey({
      settingsPda,
      signer: signerKeyId,
      args: { sessionKey: sessionKeypair.publicKey, sessionKeyExpiration: exp },
      extraVerificationData: createEvd,
      programId,
    });
    createIx.keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });

    await sendAndConfirm(nativeSigner, [createPrecompileIx, createIx]);

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

    const [proposalPda] = smartAccount.getProposalPda({ settingsPda, transactionIndex, programId });

    // Session key approves — no precompile, no EVD needed (native tx signer)
    const approveIx = smartAccount.generated.createApproveProposalV2Instruction(
      {
        consensusAccount: settingsPda,
        signer: sessionKeypair.publicKey,
        proposal: proposalPda,
        program: programId,
      },
      { args: { memo: null }, extraVerificationData: null },
      programId
    );
    // Fix isSigner
    const meta = approveIx.keys.find((k) => k.pubkey.equals(sessionKeypair.publicKey));
    if (meta) meta.isSigner = true;

    await sendAndConfirm(nativeSigner, [approveIx], [sessionKeypair]);

    // Verify: approval recorded under the PARENT external signer's key, not the session key
    const proposal = await Proposal.fromAccountAddress(connection, proposalPda);
    assert.ok(proposal.approved.length > 0, "Proposal should have approvals");
    assert.ok(
      proposal.approved[0].equals(signerKeyId),
      "Approval recorded under parent external signer's key, not session key"
    );

    // Execute the settings transaction
    const executeIx = smartAccount.instructions.executeSettingsTransaction({
      settingsPda,
      transactionIndex,
      signer: nativeSigner.publicKey,
      rentPayer: nativeSigner.publicKey,
      programId,
    });

    await sendAndConfirm(nativeSigner, [executeIx]);

    const settings = await Settings.fromAccountAddress(connection, settingsPda);
    assert.strictEqual(settings.threshold, 2, "Threshold should be changed to 2");
  });

  // =======================================================================
  // Error: session key collides with existing signer key
  // =======================================================================

  it("error: session key colliding with existing signer key_id is rejected", async () => {
    const creator = await generateFundedKeypair(connection);
    const nativeSigner = await generateFundedKeypair(connection);
    const ed25519Keypair = generateEd25519ExternalKeypair();
    const treasury = getTestProgramTreasury();
    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({ accountIndex, programId });
    const signerKeyId = new PublicKey(ed25519Keypair.publicKey);

    await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [
        {
          __kind: "Native" as const,
          key: nativeSigner.publicKey,
          permissions: { mask: Permissions.all().mask },
        },
        {
          __kind: "Ed25519External" as const,
          permissions: { mask: Permissions.all().mask },
          data: {
            externalPubkey: Array.from(ed25519Keypair.publicKey),
            sessionKeyData: { key: PublicKey.default, expiration: 0 },
          },
          nonce: 0,
        },
      ],
      timeLock: 0,
      rentCollector: null,
      programId,
    });
    await new Promise((r) => setTimeout(r, 1000));

    // Try to set the session key to the native signer's pubkey (which is an existing signer key_id)
    const exp = BigInt(Math.floor(Date.now() / 1000) + 3600);
    const collidingSessionKey = nativeSigner.publicKey; // This key already exists as a signer!

    const msg = buildSessionKeyCreateMessage(settingsPda, signerKeyId, collidingSessionKey, exp, 1n);
    const sig = signEd25519External(msg, ed25519Keypair.privateKey);
    const precompileIx = buildEd25519PrecompileInstruction(sig, ed25519Keypair.publicKey, msg);

    const evd = serializeSingleExtraVerificationData({ kind: ExtraVerificationDataKind.Ed25519Precompile });
    const ix = smartAccount.instructions.createSessionKey({
      settingsPda,
      signer: signerKeyId,
      args: { sessionKey: collidingSessionKey, sessionKeyExpiration: exp },
      extraVerificationData: evd,
      programId,
    });
    ix.keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });

    const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash();
    const txMsg = new TransactionMessage({
      payerKey: nativeSigner.publicKey,
      recentBlockhash: blockhash,
      instructions: [precompileIx, ix],
    }).compileToV0Message();
    const tx = new VersionedTransaction(txMsg);
    tx.sign([nativeSigner]);

    try {
      const txSig = await connection.sendTransaction(tx, { skipPreflight: true });
      await connection.confirmTransaction({ signature: txSig, blockhash, lastValidBlockHeight });

      const result = await connection.getTransaction(txSig, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      assert.ok(result?.meta?.err, "Transaction should have failed");
    } catch (err: any) {
      // Expected: InvalidSessionKey — session key collides with existing signer
      assert.ok(
        err.message.includes("custom program error") || err.message.includes("InvalidSessionKey"),
        `Expected InvalidSessionKey error, got: ${err.message}`
      );
    }
  });

  // =======================================================================
  // Error: native signer cannot have session keys
  // =======================================================================

  it("error: create session key for native signer fails with InvalidSignerType", async () => {
    const creator = await generateFundedKeypair(connection);
    const nativeSigner = await generateFundedKeypair(connection);
    const treasury = getTestProgramTreasury();
    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({ accountIndex, programId });

    await smartAccount.rpc.createSmartAccount({
      connection,
      treasury,
      creator,
      settings: settingsPda,
      settingsAuthority: null,
      threshold: 1,
      signers: [
        {
          __kind: "Native" as const,
          key: nativeSigner.publicKey,
          permissions: { mask: Permissions.all().mask },
        },
      ],
      timeLock: 0,
      rentCollector: null,
      programId,
    });
    await new Promise((r) => setTimeout(r, 1000));

    const sessionKeypair = Keypair.generate();
    const exp = BigInt(Math.floor(Date.now() / 1000) + 3600);

    const evd = serializeSingleExtraVerificationData({ kind: ExtraVerificationDataKind.Ed25519Precompile });
    const ix = smartAccount.instructions.createSessionKey({
      settingsPda,
      signer: nativeSigner.publicKey,
      args: { sessionKey: sessionKeypair.publicKey, sessionKeyExpiration: exp },
      extraVerificationData: evd,
      programId,
    });

    const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash();
    const msg = new TransactionMessage({
      payerKey: nativeSigner.publicKey,
      recentBlockhash: blockhash,
      instructions: [ix],
    }).compileToV0Message();
    const tx = new VersionedTransaction(msg);
    tx.sign([nativeSigner]);

    try {
      const sig = await connection.sendTransaction(tx, { skipPreflight: true });
      await connection.confirmTransaction({ signature: sig, blockhash, lastValidBlockHeight });

      const result = await connection.getTransaction(sig, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      assert.ok(result?.meta?.err, "Transaction should have failed");
    } catch (err: any) {
      // Expected: program rejects native signer for session key creation
      assert.ok(
        err.message.includes("custom program error") || err.message.includes("InvalidSignerType"),
        `Expected InvalidSignerType error, got: ${err.message}`
      );
    }
  });
});
