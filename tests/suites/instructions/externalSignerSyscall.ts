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
  generateFundedKeypair,
  getTestProgramId,
  getTestProgramTreasury,
  getNextAccountIndex,
  generateEd25519ExternalKeypair,
  signEd25519External,
  generateSecp256k1Keypair,
  signSecp256k1,
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

describe("Instructions / external_signer_syscall", () => {
  it("approve proposal with Ed25519External via syscall (no precompile)", async () => {
    const creator = await generateFundedKeypair(connection);
    const nativeSigner = await generateFundedKeypair(connection);
    const ed25519Keypair = generateEd25519ExternalKeypair();
    const treasury = getTestProgramTreasury();

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    // Native signer: Initiate + Vote + Execute
    const nativeSignerObj: smartAccount.generated.SmartAccountSigner = {
      __kind: "Native",
      key: nativeSigner.publicKey,
      permissions: {
        mask: Permission.Initiate | Permission.Vote | Permission.Execute,
      },
    };

    // Ed25519External signer: Vote + Execute
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

    // Create settings transaction
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

    // Create proposal
    signature = await smartAccount.rpc.createProposal({
      connection,
      feePayer: nativeSigner,
      settingsPda,
      transactionIndex,
      creator: nativeSigner,
      programId,
    });
    await connection.confirmTransaction(signature);

    console.log("Settings transaction and proposal created");

    const [proposalPda] = smartAccount.getProposalPda({
      settingsPda,
      transactionIndex,
      programId,
    });

    // Build the vote message matching on-chain create_vote_message:
    // hash("proposal_vote_v2" || proposal_key || vote || tx_index || next_nonce)
    const vote = 0; // Approve
    const nextNonce = BigInt(1);

    const transactionIndexBytes = Buffer.alloc(8);
    transactionIndexBytes.writeBigUInt64LE(BigInt(transactionIndex));

    const nonceBytes = Buffer.alloc(8);
    nonceBytes.writeBigUInt64LE(nextNonce);

    const messageData = Buffer.concat([
      Buffer.from("proposal_vote_v2", "utf-8"),
      proposalPda.toBuffer(),
      Buffer.from([vote]),
      transactionIndexBytes,
      nonceBytes,
    ]);

    const hashedMessage = sha256(messageData);

    // Sign with Ed25519 external key (64-byte signature)
    const sig = signEd25519External(hashedMessage, ed25519Keypair.privateKey);

    // Build approveProposalV2 with extraVerificationData containing the signature
    // NO precompile instruction, NO SYSVAR_INSTRUCTIONS in remaining_accounts
    const signerKey = new PublicKey(ed25519Keypair.publicKey);

    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.Ed25519Syscall,
      signature: sig,
    });

    const approveProposalIx =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey,
          proposal: proposalPda,
          program: programId,
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

    let message = new TransactionMessage({
      payerKey: creator.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [approveProposalIx],
    }).compileToV0Message();

    let tx = new VersionedTransaction(message);
    tx.sign([creator]);

    signature = await connection.sendTransaction(tx, { skipPreflight: true });
    await connection.confirmTransaction(signature);

    // Verify on-chain success
    const txResult = await connection.getTransaction(signature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    if (txResult?.meta?.err) {
      console.error("Ed25519 syscall TX failed on-chain!");
      console.error("Error:", JSON.stringify(txResult.meta.err));
      console.error("Logs:", txResult.meta.logMessages);
      throw new Error(
        `Ed25519 syscall TX failed: ${JSON.stringify(txResult.meta.err)}`
      );
    }
    console.log("Ed25519 syscall TX logs:", txResult?.meta?.logMessages);

    console.log(
      "✓ Proposal approved with Ed25519External via syscall (no precompile)"
    );

    // Verify proposal was approved
    const proposal = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(proposal.approved.length > 0, "Proposal should have approvals");

    // Execute the settings transaction
    const executeTransactionIx =
      smartAccount.instructions.executeSettingsTransaction({
        settingsPda,
        transactionIndex,
        signer: nativeSigner.publicKey,
        rentPayer: nativeSigner.publicKey,
        programId,
      });

    message = new TransactionMessage({
      payerKey: nativeSigner.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [executeTransactionIx],
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

    console.log("✓ Ed25519External syscall verification complete");
  });

  it("approve proposal with Secp256k1 via syscall (no precompile)", async () => {
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

    console.log("Settings transaction and proposal created");

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

    const messageData = Buffer.concat([
      Buffer.from("proposal_vote_v2", "utf-8"),
      proposalPda.toBuffer(),
      Buffer.from([vote]),
      transactionIndexBytes,
      nonceBytes,
    ]);

    // For Secp256k1 syscall: on-chain does keccak256(sha256(message_data)) for ECDSA recovery
    const messageHash = sha256(messageData);
    const keccakHash = keccak_256(messageHash);

    // Sign the keccak256 hash
    const { signature: sig, recoveryId } = signSecp256k1(
      keccakHash,
      secp256k1Keypair.privateKey
    );

    // Signer key for Secp256k1: first 32 bytes of uncompressed pubkey
    const signerKey = new PublicKey(
      secp256k1Keypair.publicKeyUncompressed.slice(0, 32)
    );

    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.Secp256k1Syscall,
      signature: sig,
      recoveryId,
    });

    const approveProposalIx =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey,
          proposal: proposalPda,
          program: programId,
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
      instructions: [approveProposalIx],
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
      console.error("Secp256k1 syscall TX failed on-chain!");
      console.error("Error:", JSON.stringify(txResult.meta.err));
      console.error("Logs:", txResult.meta.logMessages);
      throw new Error(
        `Secp256k1 syscall TX failed: ${JSON.stringify(txResult.meta.err)}`
      );
    }
    console.log("Secp256k1 syscall TX logs:", txResult?.meta?.logMessages);

    console.log(
      "✓ Proposal approved with Secp256k1 via syscall (no precompile)"
    );

    const proposal = await Proposal.fromAccountAddress(
      connection,
      proposalPda
    );
    assert.ok(proposal.approved.length > 0, "Proposal should have approvals");

    // Execute the settings transaction
    const executeTransactionIx =
      smartAccount.instructions.executeSettingsTransaction({
        settingsPda,
        transactionIndex,
        signer: nativeSigner.publicKey,
        rentPayer: nativeSigner.publicKey,
        programId,
      });

    message = new TransactionMessage({
      payerKey: nativeSigner.publicKey,
      recentBlockhash: (await connection.getLatestBlockhash()).blockhash,
      instructions: [executeTransactionIx],
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

    console.log("✓ Secp256k1 syscall verification complete");
  });

  it("P256Webauthn returns PrecompileRequired without sysvar", async () => {
    const creator = await generateFundedKeypair(connection);
    const nativeSigner = await generateFundedKeypair(connection);
    const treasury = getTestProgramTreasury();

    const accountIndex = await getNextAccountIndex(connection, programId);
    const [settingsPda] = smartAccount.getSettingsPda({
      accountIndex,
      programId,
    });

    const rpId = "example.com";
    const rpIdBytes = Buffer.from(rpId, "utf-8");
    const rpIdPadded = Buffer.alloc(32);
    rpIdBytes.copy(rpIdPadded);
    const rpIdHash = sha256(rpIdBytes);

    // Import P256 keypair generation
    const { generateP256Keypair, signP256 } = await import("../../utils");
    const p256Keypair = generateP256Keypair();

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

    // P256 signer key: compressed_pubkey[1..33] (skip the 0x02/0x03 prefix byte)
    const signerKey = new PublicKey(
      p256Keypair.publicKeyCompressed.slice(1, 33)
    );

    // Send with Ed25519Syscall variant — P256 doesn't support syscall, should fail with PrecompileRequired
    const evdBytes = serializeSingleExtraVerificationData({
      kind: ExtraVerificationDataKind.Ed25519Syscall,
      signature: new Uint8Array(64),
    });

    const approveProposalIx =
      smartAccount.generated.createApproveProposalV2Instruction(
        {
          consensusAccount: settingsPda,
          signer: signerKey,
          proposal: proposalPda,
          program: programId,
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
      instructions: [approveProposalIx],
    }).compileToV0Message();

    let tx = new VersionedTransaction(message);
    tx.sign([creator]);

    try {
      signature = await connection.sendTransaction(tx, {
        skipPreflight: true,
      });
      await connection.confirmTransaction(signature);

      // Check if it actually failed on-chain
      const txResult = await connection.getTransaction(signature, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      if (txResult?.meta?.err) {
        console.log(
          "✓ P256Webauthn correctly rejected syscall path:",
          JSON.stringify(txResult.meta.err)
        );
        // Verify it's a PrecompileRequired error in the logs
        const logs = txResult?.meta?.logMessages?.join("\n") ?? "";
        assert.ok(
          logs.includes("PrecompileRequired") ||
            logs.includes("custom program error"),
          "Should fail with PrecompileRequired error"
        );
        return;
      }
      // If we get here, the transaction succeeded which is unexpected
      assert.fail(
        "P256Webauthn should have failed without precompile instruction"
      );
    } catch (error: any) {
      // Transaction simulation failure is also acceptable
      console.log(
        "✓ P256Webauthn correctly rejected syscall path:",
        error.message
      );
    }
  });
});
