import * as borsh from "borsh";

/**
 * ExtraVerificationData enum discriminators.
 * Must match Rust enum ordering exactly.
 */
export enum ExtraVerificationDataKind {
  P256WebauthnPrecompile = 0,
  Ed25519Precompile = 1,
  Secp256k1Precompile = 2,
  P256NativePrecompile = 3,
  Ed25519Syscall = 4,
  Secp256k1Syscall = 5,
}

export type ExtraVerificationData =
  | {
      kind: ExtraVerificationDataKind.P256WebauthnPrecompile;
      typeAndFlags: number;
      port: number;
    }
  | { kind: ExtraVerificationDataKind.Ed25519Precompile }
  | { kind: ExtraVerificationDataKind.Secp256k1Precompile }
  | { kind: ExtraVerificationDataKind.P256NativePrecompile }
  | {
      kind: ExtraVerificationDataKind.Ed25519Syscall;
      signature: Uint8Array; // 64 bytes
    }
  | {
      kind: ExtraVerificationDataKind.Secp256k1Syscall;
      signature: Uint8Array; // 64 bytes
      recoveryId: number;
    };

/**
 * Serialize a single ExtraVerificationData variant to bytes.
 * Used by async instruction handlers.
 */
export function serializeSingleExtraVerificationData(
  evd: ExtraVerificationData
): Uint8Array {
  return serializeVariant(evd);
}

/**
 * Serialize a SmallVec<u8, ExtraVerificationData> to bytes.
 * Used by sync instruction handlers (multiple external signers).
 * Format: SmallVec<u8> = [1-byte length][variant_0][variant_1]...
 */
export function serializeExtraVerificationDataVec(
  variants: ExtraVerificationData[]
): Uint8Array {
  const serializedVariants = variants.map(serializeVariant);
  const totalDataLen = serializedVariants.reduce(
    (acc, v) => acc + v.length,
    0
  );

  const buf = Buffer.alloc(1 + totalDataLen);
  buf.writeUInt8(variants.length, 0);

  let offset = 1;
  for (const s of serializedVariants) {
    buf.set(s, offset);
    offset += s.length;
  }

  return buf;
}

function serializeVariant(evd: ExtraVerificationData): Uint8Array {
  switch (evd.kind) {
    case ExtraVerificationDataKind.P256WebauthnPrecompile: {
      const buf = Buffer.alloc(1 + 3); // disc + typeAndFlags(1) + port(2)
      buf.writeUInt8(evd.kind, 0);
      buf.writeUInt8(evd.typeAndFlags, 1);
      buf.writeUInt16LE(evd.port, 2);
      return buf;
    }
    case ExtraVerificationDataKind.Ed25519Precompile: {
      return Buffer.from([evd.kind]);
    }
    case ExtraVerificationDataKind.Secp256k1Precompile: {
      return Buffer.from([evd.kind]);
    }
    case ExtraVerificationDataKind.P256NativePrecompile: {
      return Buffer.from([evd.kind]);
    }
    case ExtraVerificationDataKind.Ed25519Syscall: {
      const buf = Buffer.alloc(1 + 64); // disc + signature
      buf.writeUInt8(evd.kind, 0);
      buf.set(evd.signature, 1);
      return buf;
    }
    case ExtraVerificationDataKind.Secp256k1Syscall: {
      const buf = Buffer.alloc(1 + 64 + 1); // disc + signature + recovery_id
      buf.writeUInt8(evd.kind, 0);
      buf.set(evd.signature, 1);
      buf.writeUInt8(evd.recoveryId, 65);
      return buf;
    }
  }
}
