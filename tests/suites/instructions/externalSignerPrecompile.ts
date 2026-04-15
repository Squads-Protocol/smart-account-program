import * as smartAccount from "@sqds/smart-account";
import {
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
} from "../../utils";
import {
  ExtraVerificationDataKind,
  serializeSingleExtraVerificationData,
} from "../../helpers/extraVerificationData";
import { sha256 } from "@noble/hashes/sha256";
import { keccak_256 } from "@noble/hashes/sha3";

const { Settings, Proposal } = smartAccount.accounts;
const { Permission } = smartAccount.types;

const programId = getTestProgramId();
const connection = createLocalhostConnection();

describe("Instructions / external_signer_precompile", () => {
  it("approve proposal with Ed25519External via precompile", async () => {
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
        sessionKeyData: {
          key: PublicKey.default,
          expiration: 0,
        },
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

    // Build vote message: sha256("proposal_vote_v2" || proposal || vote || tx_index || nonce)
    const vote = 0; // Approve
    const nextNonce = BigInt(1);
    const transactionIndexBytes = Buffer.alloc(8);
    transactionIndexBytes.writeBigUInt64LE(BigInt(transactionIndex));
    const nonceBytes = Buffer.alloc(8);
    nonceBytes.writeBigUInt64LE(nextNonce);

    const hashedMessage = sha256(
      Buffer.concat([
        Buffer.from("proposal_vote_v2", "utf-8"),
        proposalPda.toBuffer(),
        Buffer.from([vote]),
        transactionIndexBytes,
        nonceBytes,
      ])
    );

    // Sign with Ed25519 external key
    const sig = signEd25519External(hashedMessage, ed25519Keypair.privateKey);

    // Build Ed25519 precompile instruction at index 0
    const precompileIx = buildEd25519PrecompileInstruction(
      sig,
      ed25519Keypair.publicKey,
      hashedMessage
    );

    const signerKey = new PublicKey(ed25519Keypair.publicKey);

    // Build approveProposalV2 with SYSVAR_INSTRUCTIONS in remaining accounts (precompile path)
    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.Ed25519Precompile,
    });

    const approveProposalIx =
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
        {
          args: { memo: null },
          extraVerificationData: null,
        },
        programId
      );

    // Patch EVD: replace trailing Option::None (0x00) with Option::Some + raw EVD bytes
    const withoutNull1 = approveProposalIx.data.subarray(0, approveProposalIx.data.length - 1);
    approveProposalIx.data = Buffer.concat([withoutNull1, Buffer.from([0x01]), Buffer.from(evdBytes)]);

    // Transaction: [precompileIx @ 0, approveProposalV2Ix @ 1]
    let message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx, approveProposalIx],
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

    // Execute the settings transaction
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

  it("approve proposal with Secp256k1 via precompile", async () => {
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
        sessionKeyData: {
          key: PublicKey.default,
          expiration: 0,
        },
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

    // Build vote message
    const vote = 0;
    const nextNonce = BigInt(1);
    const transactionIndexBytes = Buffer.alloc(8);
    transactionIndexBytes.writeBigUInt64LE(BigInt(transactionIndex));
    const nonceBytes = Buffer.alloc(8);
    nonceBytes.writeBigUInt64LE(nextNonce);

    const hashedMessage = sha256(
      Buffer.concat([
        Buffer.from("proposal_vote_v2", "utf-8"),
        proposalPda.toBuffer(),
        Buffer.from([vote]),
        transactionIndexBytes,
        nonceBytes,
      ])
    );

    // For secp256k1 precompile: sign keccak256(hashedMessage)
    // The precompile receives hashedMessage as raw bytes and internally computes
    // keccak256(hashedMessage) for ECDSA recovery.
    const keccakHash = keccak_256(hashedMessage);
    const { signature: sig, recoveryId } = signSecp256k1(
      keccakHash,
      secp256k1Keypair.privateKey
    );

    // Build secp256k1 precompile instruction at index 0.
    // messageHash parameter = hashedMessage (the precompile will keccak256 it internally).
    const precompileIx = buildSecp256k1PrecompileInstruction(
      sig,
      recoveryId,
      hashedMessage,
      secp256k1Keypair.ethAddress,
      0 // instruction index in transaction
    );

    // Secp256k1 signer key: first 32 bytes of uncompressed pubkey
    const signerKey = new PublicKey(
      secp256k1Keypair.publicKeyUncompressed.slice(0, 32)
    );

    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.Secp256k1Precompile,
    });

    const approveProposalIx =
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
        {
          args: { memo: null },
          extraVerificationData: null,
        },
        programId
      );

    // Patch EVD: replace trailing Option::None (0x00) with Option::Some + raw EVD bytes
    const withoutNull2 = approveProposalIx.data.subarray(0, approveProposalIx.data.length - 1);
    approveProposalIx.data = Buffer.concat([withoutNull2, Buffer.from([0x01]), Buffer.from(evdBytes)]);

    let message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx, approveProposalIx],
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
      console.error(
        "Secp256k1 precompile TX logs:",
        txResult.meta.logMessages
      );
      throw new Error(
        `Secp256k1 precompile TX failed: ${JSON.stringify(txResult.meta.err)}`
      );
    }

    const proposal = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(proposal.approved.length > 0, "Proposal should have approvals");

    // Execute the settings transaction
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

  it("approve proposal with P256Webauthn via precompile", async () => {
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
        sessionKeyData: {
          key: PublicKey.default,
          expiration: 0,
        },
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

    // Build vote message
    const vote = 0;
    const nextNonce = BigInt(1);
    const transactionIndexBytes = Buffer.alloc(8);
    transactionIndexBytes.writeBigUInt64LE(BigInt(transactionIndex));
    const nonceBytes = Buffer.alloc(8);
    nonceBytes.writeBigUInt64LE(nextNonce);

    const hashedMessage = sha256(
      Buffer.concat([
        Buffer.from("proposal_vote_v2", "utf-8"),
        proposalPda.toBuffer(),
        Buffer.from([vote]),
        transactionIndexBytes,
        nonceBytes,
      ])
    );

    // Build WebAuthn signature: authenticatorData || clientDataHash
    const counter = 1;
    const counterBuf = Buffer.alloc(4);
    counterBuf.writeUInt32BE(counter);
    const authenticatorData = Buffer.concat([
      Buffer.from(rpIdHash),
      Buffer.from([0x01]), // flags: UP
      counterBuf,
    ]);

    const challengeB64url = base64urlEncode(hashedMessage);
    const clientDataJSON = `{"type":"webauthn.get","challenge":"${challengeB64url}","origin":"https://${rpId}","crossOrigin":false}`;
    const clientDataHash = sha256(Buffer.from(clientDataJSON, "utf-8"));

    const precompileMessage = Buffer.concat([
      authenticatorData,
      Buffer.from(clientDataHash),
    ]);

    // Sign with P256 key (prehash: true does SHA-256 internally, matching the precompile)
    const p256Sig = signP256(precompileMessage, p256Keypair.privateKey);

    // Build secp256r1 precompile instruction at index 0
    const precompileIx = buildSecp256r1PrecompileInstruction(
      p256Sig,
      p256Keypair.publicKeyCompressed,
      precompileMessage
    );

    // P256 signer key: compressed_pubkey[0..32] (first 32 bytes)
    const signerKey = new PublicKey(
      p256Keypair.publicKeyCompressed.slice(0, 32)
    );

    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.P256WebauthnPrecompile,
      typeAndFlags: 0x10, // TYPE_GET
      port: 0,
    });

    const approveProposalIx =
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
        {
          args: { memo: null },
          extraVerificationData: null,
        },
        programId
      );

    // Patch EVD: replace trailing Option::None (0x00) with Option::Some + raw EVD bytes
    const withoutNull3 = approveProposalIx.data.subarray(0, approveProposalIx.data.length - 1);
    approveProposalIx.data = Buffer.concat([withoutNull3, Buffer.from([0x01]), Buffer.from(evdBytes)]);

    let message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [precompileIx, approveProposalIx],
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

    // Execute the settings transaction
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
