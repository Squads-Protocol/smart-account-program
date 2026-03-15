import { PublicKey, TransactionInstruction } from "@solana/web3.js";
import { ed25519 } from "@noble/curves/ed25519";
import { secp256k1 } from "@noble/curves/secp256k1";
import { p256 } from "@noble/curves/p256";
import { keccak_256 } from "@noble/hashes/sha3";
import { sha256 } from "@noble/hashes/sha256";

/**
 * Base64url encoding (no padding) — used for WebAuthn clientDataJSON challenges.
 */
export function base64urlEncode(input: Uint8Array): string {
  const alphabet =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  let output = "";
  for (let i = 0; i < input.length; i += 3) {
    const b0 = input[i];
    const b1 = i + 1 < input.length ? input[i + 1] : 0;
    const b2 = i + 2 < input.length ? input[i + 2] : 0;
    output += alphabet[b0 >> 2];
    output += alphabet[((b0 & 3) << 4) | (b1 >> 4)];
    if (i + 1 < input.length) output += alphabet[((b1 & 15) << 2) | (b2 >> 6)];
    if (i + 2 < input.length) output += alphabet[b2 & 63];
  }
  return output;
}

/**
 * Build the sha256 message hash for proposal_vote_v2.
 */
export function buildVoteMessage(
  proposalPda: PublicKey,
  vote: number,
  transactionIndex: bigint,
  nextNonce: bigint
): Uint8Array {
  const txIndexBytes = Buffer.alloc(8);
  txIndexBytes.writeBigUInt64LE(transactionIndex);
  const nonceBytes = Buffer.alloc(8);
  nonceBytes.writeBigUInt64LE(nextNonce);

  return sha256(
    Buffer.concat([
      Buffer.from("proposal_vote_v2", "utf-8"),
      proposalPda.toBuffer(),
      Buffer.from([vote]),
      txIndexBytes,
      nonceBytes,
    ])
  );
}

/**
 * Ed25519External signer utilities
 */
export interface Ed25519ExternalKeypair {
  publicKey: Uint8Array; // 32 bytes
  privateKey: Uint8Array; // 32 bytes
}

export function generateEd25519ExternalKeypair(): Ed25519ExternalKeypair {
  const privateKey = ed25519.utils.randomPrivateKey();
  const publicKey = ed25519.getPublicKey(privateKey);
  return { publicKey, privateKey };
}

export function signEd25519External(
  message: Uint8Array,
  privateKey: Uint8Array
): Uint8Array {
  return ed25519.sign(message, privateKey);
}

/**
 * Secp256k1 signer utilities
 */
export interface Secp256k1Keypair {
  publicKeyUncompressed: Uint8Array; // 64 bytes (no prefix)
  privateKey: Uint8Array; // 32 bytes
  ethAddress: Uint8Array; // 20 bytes
}

export function generateSecp256k1Keypair(): Secp256k1Keypair {
  const privateKey = secp256k1.utils.randomPrivateKey();
  const publicKeyCompressed = secp256k1.getPublicKey(privateKey, true);
  const publicKeyUncompressed = secp256k1.getPublicKey(privateKey, false).slice(1); // Remove 0x04 prefix

  // Compute Ethereum address: keccak256(uncompressed_pubkey)[12:]
  const hash = keccak_256(publicKeyUncompressed);
  const ethAddress = hash.slice(12);

  return {
    publicKeyUncompressed,
    privateKey,
    ethAddress,
  };
}

export function signSecp256k1(
  messageHash: Uint8Array,
  privateKey: Uint8Array
): { signature: Uint8Array; recoveryId: number } {
  const sig = secp256k1.sign(messageHash, privateKey);
  return {
    signature: sig.toCompactRawBytes(),
    recoveryId: sig.recovery!,
  };
}

/**
 * P256 (secp256r1) WebAuthn signer utilities
 */
export interface P256WebauthnKeypair {
  publicKeyCompressed: Uint8Array; // 33 bytes
  privateKey: Uint8Array; // 32 bytes
}

export function generateP256Keypair(): P256WebauthnKeypair {
  const privateKey = p256.utils.randomPrivateKey();
  const publicKeyCompressed = p256.getPublicKey(privateKey, true);
  return {
    publicKeyCompressed,
    privateKey,
  };
}

export function signP256(
  message: Uint8Array,
  privateKey: Uint8Array
): Uint8Array {
  // prehash: true makes noble SHA-256 the message before ECDSA signing,
  // matching the Solana secp256r1 precompile which also SHA-256 hashes the message.
  // lowS: true is REQUIRED — the Solana precompile enforces s <= half_order.
  const sig = p256.sign(message, privateKey, { prehash: true, lowS: true });
  return sig.toCompactRawBytes(); // 64 bytes (r + s)
}

/**
 * Build precompile instructions for Solana
 */

export function buildEd25519PrecompileInstruction(
  signature: Uint8Array, // 64 bytes
  publicKey: Uint8Array, // 32 bytes
  message: Uint8Array
): TransactionInstruction {
  const ED25519_PROGRAM_ID = new PublicKey(
    "Ed25519SigVerify111111111111111111111111111"
  );

  // Ed25519 instruction data format:
  // [num_signatures: u8][padding: u8][signature_offset: u16 LE][signature_instruction_index: u16 LE]
  // [public_key_offset: u16 LE][public_key_instruction_index: u16 LE]
  // [message_data_offset: u16 LE][message_data_size: u16 LE][message_instruction_index: u16 LE]
  // [signature: 64 bytes][public_key: 32 bytes][message: variable]

  const numSignatures = 1;
  const signatureOffset = 2 + (numSignatures * 14); // Header + offsets
  const publicKeyOffset = signatureOffset + 64;
  const messageOffset = publicKeyOffset + 32;

  const data = Buffer.alloc(signatureOffset + 64 + 32 + message.length);
  let offset = 0;

  // Header
  data.writeUInt8(numSignatures, offset); offset += 1;
  data.writeUInt8(0, offset); offset += 1; // padding

  // Signature offsets
  data.writeUInt16LE(signatureOffset, offset); offset += 2;
  data.writeUInt16LE(0xFFFF, offset); offset += 2; // signature_instruction_index (same ix)

  // Public key offsets
  data.writeUInt16LE(publicKeyOffset, offset); offset += 2;
  data.writeUInt16LE(0xFFFF, offset); offset += 2;

  // Message offsets
  data.writeUInt16LE(messageOffset, offset); offset += 2;
  data.writeUInt16LE(message.length, offset); offset += 2;
  data.writeUInt16LE(0xFFFF, offset); offset += 2;

  // Data
  data.set(signature, signatureOffset);
  data.set(publicKey, publicKeyOffset);
  data.set(message, messageOffset);

  return new TransactionInstruction({
    programId: ED25519_PROGRAM_ID,
    keys: [],
    data,
  });
}

export function buildSecp256k1PrecompileInstruction(
  signature: Uint8Array, // 64 bytes
  recoveryId: number, // 0-3
  messageHash: Uint8Array, // 32 bytes (keccak256 hash)
  ethAddress: Uint8Array, // 20 bytes
  instructionIndex: number = 0 // index of this instruction in the transaction
): TransactionInstruction {
  const SECP256K1_PROGRAM_ID = new PublicKey(
    "KeccakSecp256k11111111111111111111111111111"
  );

  // Secp256k1 instruction format:
  // [num_signatures: u8][signature_offset: u16 LE][signature_instruction_index: u8]
  // [eth_address_offset: u16 LE][eth_address_instruction_index: u8]
  // [message_data_offset: u16 LE][message_data_size: u16 LE][message_instruction_index: u8]
  // [signature: 65 bytes (64 + recovery_id)][eth_address: 20 bytes][message: 32 bytes]

  const numSignatures = 1;
  const signatureOffset = 1 + (numSignatures * 11);
  const ethAddressOffset = signatureOffset + 65;
  const messageOffset = ethAddressOffset + 20;

  const data = Buffer.alloc(messageOffset + 32);
  let offset = 0;

  // Header
  data.writeUInt8(numSignatures, offset); offset += 1;

  // Signature offsets (u8 instruction index = position of this ix in transaction)
  data.writeUInt16LE(signatureOffset, offset); offset += 2;
  data.writeUInt8(instructionIndex, offset); offset += 1;

  // Eth address offsets
  data.writeUInt16LE(ethAddressOffset, offset); offset += 2;
  data.writeUInt8(instructionIndex, offset); offset += 1;

  // Message offsets
  data.writeUInt16LE(messageOffset, offset); offset += 2;
  data.writeUInt16LE(32, offset); offset += 2;
  data.writeUInt8(instructionIndex, offset); offset += 1;

  // Data: signature (64 bytes + recovery_id)
  data.set(signature, signatureOffset);
  data.writeUInt8(recoveryId, signatureOffset + 64);

  // Eth address
  data.set(ethAddress, ethAddressOffset);

  // Message hash
  data.set(messageHash, messageOffset);

  return new TransactionInstruction({
    programId: SECP256K1_PROGRAM_ID,
    keys: [],
    data,
  });
}

export function buildSecp256r1PrecompileInstruction(
  signature: Uint8Array, // 64 bytes (r + s)
  publicKeyCompressed: Uint8Array, // 33 bytes
  message: Uint8Array // variable length message
): TransactionInstruction {
  const SECP256R1_PROGRAM_ID = new PublicKey(
    "Secp256r1SigVerify1111111111111111111111111"
  );

  // Secp256r1 instruction format:
  // [num_signatures: u8][padding: u8][SignatureOffsets (14 bytes)]
  // [public_key: 33 bytes][signature: 64 bytes][message: variable]

  const numSignatures = 1;
  const signatureOffset = 2 + (numSignatures * 14); // 2-byte header + offsets
  const publicKeyOffset = signatureOffset + 64;
  const messageOffset = publicKeyOffset + 33;

  const data = Buffer.alloc(messageOffset + message.length);
  let offset = 0;

  // Header (2 bytes: num_signatures + padding)
  data.writeUInt8(numSignatures, offset); offset += 1;
  data.writeUInt8(0, offset); offset += 1; // padding

  // Signature offsets
  data.writeUInt16LE(signatureOffset, offset); offset += 2;
  data.writeUInt16LE(0xFFFF, offset); offset += 2;

  // Public key offsets
  data.writeUInt16LE(publicKeyOffset, offset); offset += 2;
  data.writeUInt16LE(0xFFFF, offset); offset += 2;

  // Message offsets
  data.writeUInt16LE(messageOffset, offset); offset += 2;
  data.writeUInt16LE(message.length, offset); offset += 2;
  data.writeUInt16LE(0xFFFF, offset); offset += 2;

  // Data
  data.set(signature, signatureOffset);
  data.set(publicKeyCompressed, publicKeyOffset);
  data.set(message, messageOffset);

  return new TransactionInstruction({
    programId: SECP256R1_PROGRAM_ID,
    keys: [],
    data,
  });
}

export function buildSecp256r1MultiSigPrecompileInstruction(
  signers: {
    signature: Uint8Array; // 64 bytes (r + s)
    publicKeyCompressed: Uint8Array; // 33 bytes
    message: Uint8Array; // variable length
  }[]
): TransactionInstruction {
  const SECP256R1_PROGRAM_ID = new PublicKey(
    "Secp256r1SigVerify1111111111111111111111111"
  );

  const numSignatures = signers.length;
  const headerSize = 2 + numSignatures * 14; // 2-byte header + 14 bytes per SignatureOffsets

  // Per-signer data blocks: [pubkey(33) || signature(64) || message(var)] for each signer.
  // The secp256r1 precompile requires this contiguous per-signer layout.
  const dataBlocks = signers.map(s =>
    Buffer.concat([Buffer.from(s.publicKeyCompressed), Buffer.from(s.signature), Buffer.from(s.message)])
  );

  const totalSize = headerSize + dataBlocks.reduce((sum, b) => sum + b.length, 0);
  const data = Buffer.alloc(totalSize);
  let offset = 0;

  // Header
  data.writeUInt8(numSignatures, offset); offset += 1;
  data.writeUInt8(0, offset); offset += 1; // padding

  // Per-signature offset blocks (14 bytes each)
  let blockOffset = headerSize;
  for (let i = 0; i < numSignatures; i++) {
    const pkOffset = blockOffset;
    const sigOffset = blockOffset + 33;
    const msgOffset = blockOffset + 33 + 64;
    const msgLen = signers[i].message.length;

    data.writeUInt16LE(sigOffset, offset); offset += 2;
    data.writeUInt16LE(0xFFFF, offset); offset += 2; // signature_instruction_index

    data.writeUInt16LE(pkOffset, offset); offset += 2;
    data.writeUInt16LE(0xFFFF, offset); offset += 2; // pubkey_instruction_index

    data.writeUInt16LE(msgOffset, offset); offset += 2;
    data.writeUInt16LE(msgLen, offset); offset += 2;
    data.writeUInt16LE(0xFFFF, offset); offset += 2; // message_instruction_index

    blockOffset += 33 + 64 + msgLen;
  }

  // Write per-signer data blocks
  let dataOffset = headerSize;
  for (const block of dataBlocks) {
    block.copy(data, dataOffset);
    dataOffset += block.length;
  }

  return new TransactionInstruction({
    programId: SECP256R1_PROGRAM_ID,
    keys: [],
    data,
  });
}
