import * as beet from "@metaplex-foundation/beet";
import * as beetSolana from "@metaplex-foundation/beet-solana";
import { PublicKey } from "@solana/web3.js";
import invariant from "invariant";
import { Permissions as IPermissions } from "./generated";

export {
  isProposalStatusActive,
  isProposalStatusApproved,
  isProposalStatusRejected,
  isProposalStatusCancelled,
  isProposalStatusExecuted,
  SmartAccountSigner,
  SettingsAction,
  isSettingsActionAddSigner,
  isSettingsActionRemoveSigner,
  isSettingsActionChangeThreshold,
  isSettingsActionAddSpendingLimit,
  isSettingsActionRemoveSpendingLimit,
  isSettingsActionSetTimeLock,
  SettingsActionRecord,
  Period,
  SmartAccountSignerWrapper,
  isSmartAccountSignerWrapperV1,
  isSmartAccountSignerWrapperV2,
  LegacySmartAccountSigner,
} from "./generated";

export const Permission = {
  Initiate: 0b0000_0001,
  Vote: 0b0000_0010,
  Execute: 0b0000_0100,
} as const;

export type Permission = typeof Permission[keyof typeof Permission];

export class Permissions implements IPermissions {
  private constructor(readonly mask: number) {}

  static fromPermissions(permissions: Permission[]) {
    return new Permissions(
      permissions.reduce((mask, permission) => mask | permission, 0)
    );
  }

  static all() {
    return new Permissions(
      Object.values(Permission).reduce(
        (mask, permission) => mask | permission,
        0
      )
    );
  }

  static has(permissions: IPermissions, permission: Permission) {
    return (permissions.mask & permission) === permission;
  }
}

/**
 * De/Serializes a small array with configurable length prefix and a specific number of elements of type {@link T}
 * which do not all have the same size.
 *
 * @template T type of elements held in the array
 *
 * @param lengthBeet the De/Serializer for the array length prefix
 * @param elements the De/Serializers for the element types
 * @param elementsByteSize size of all elements in the array combined
 *
 * The implementation is minor modification of `fixedSizeArray` where the length is encoded as `lengthBeet.byteSize` bytes:
 * https://github.dev/metaplex-foundation/beet/blob/e053b7b5b0c46ce7f6906ecd38be9fd85d6e5254/beet/src/beets/collections.ts#L84
 */
export function fixedSizeSmallArray<T, V = Partial<T>>(
  lengthBeet: beet.FixedSizeBeet<number>,
  elements: beet.FixedSizeBeet<T, V>[],
  elementsByteSize: number
): beet.FixedSizeBeet<T[], V[]> {
  const len = elements.length;
  const firstElement = len === 0 ? "<EMPTY>" : elements[0].description;

  return {
    write: function (buf: Buffer, offset: number, value: V[]): void {
      invariant(
        value.length === len,
        `array length ${value.length} should match len ${len}`
      );
      lengthBeet.write(buf, offset, len);

      let cursor = offset + lengthBeet.byteSize;
      for (let i = 0; i < len; i++) {
        const element = elements[i];
        element.write(buf, cursor, value[i]);
        cursor += element.byteSize;
      }
    },

    read: function (buf: Buffer, offset: number): T[] {
      const size = lengthBeet.read(buf, offset);
      invariant(size === len, "invalid byte size");

      let cursor = offset + lengthBeet.byteSize;
      const arr: T[] = new Array(len);
      for (let i = 0; i < len; i++) {
        const element = elements[i];
        arr[i] = element.read(buf, cursor);
        cursor += element.byteSize;
      }
      return arr;
    },
    byteSize: lengthBeet.byteSize + elementsByteSize,
    length: len,
    description: `Array<${firstElement}>(${len})[ ${lengthBeet.byteSize} + ${elementsByteSize} ]`,
  };
}

/**
 * Wraps a small array De/Serializer with configurable length prefix and elements of type {@link T}
 * which do not all have the same size.
 *
 * @template T type of elements held in the array
 *
 * @param lengthBeet the De/Serializer for the array length prefix
 * @param element the De/Serializer for the element types
 *
 * The implementation is minor modification of `array` where the length is encoded as `lengthBeet.byteSize` bytes:
 * https://github.dev/metaplex-foundation/beet/blob/e053b7b5b0c46ce7f6906ecd38be9fd85d6e5254/beet/src/beets/collections.ts#L137
 */
export function smallArray<T, V = Partial<T>>(
  lengthBeet: beet.FixedSizeBeet<number>,
  element: beet.Beet<T, V>
): beet.FixableBeet<T[], V[]> {
  return {
    toFixedFromData(buf: Buffer, offset: number): beet.FixedSizeBeet<T[], V[]> {
      const len = lengthBeet.read(buf, offset);
      const cursorStart = offset + lengthBeet.byteSize;
      let cursor = cursorStart;

      const fixedElements: beet.FixedSizeBeet<T, V>[] = new Array(len);
      for (let i = 0; i < len; i++) {
        const fixedElement = beet.fixBeetFromData(
          element,
          buf,
          cursor
        ) as beet.FixedSizeBeet<T, V>;
        fixedElements[i] = fixedElement;
        cursor += fixedElement.byteSize;
      }
      return fixedSizeSmallArray(
        lengthBeet,
        fixedElements,
        cursor - cursorStart
      );
    },

    toFixedFromValue(vals: V[]): beet.FixedSizeBeet<T[], V[]> {
      invariant(Array.isArray(vals), `${vals} should be an array`);

      let elementsSize = 0;
      const fixedElements: beet.FixedSizeBeet<T, V>[] = new Array(vals.length);

      for (let i = 0; i < vals.length; i++) {
        const fixedElement: beet.FixedSizeBeet<T, V> = beet.fixBeetFromValue<
          T,
          V
        >(element, vals[i]);
        fixedElements[i] = fixedElement;
        elementsSize += fixedElement.byteSize;
      }
      return fixedSizeSmallArray(lengthBeet, fixedElements, elementsSize);
    },

    description: `smallArray`,
  };
}

export type CompiledMsInstruction = {
  programIdIndex: number;
  accountIndexes: number[];
  data: number[];
};

export const compiledMsInstructionBeet =
  new beet.FixableBeetArgsStruct<CompiledMsInstruction>(
    [
      ["programIdIndex", beet.u8],
      ["accountIndexes", smallArray(beet.u8, beet.u8)],
      ["data", smallArray(beet.u16, beet.u8)],
    ],
    "CompiledMsInstruction"
  );

export type MessageAddressTableLookup = {
  /** Address lookup table account key */
  accountKey: PublicKey;
  /** List of indexes used to load writable account addresses */
  writableIndexes: number[];
  /** List of indexes used to load readonly account addresses */
  readonlyIndexes: number[];
};

export const messageAddressTableLookupBeet =
  new beet.FixableBeetArgsStruct<MessageAddressTableLookup>(
    [
      ["accountKey", beetSolana.publicKey],
      ["writableIndexes", smallArray(beet.u8, beet.u8)],
      ["readonlyIndexes", smallArray(beet.u8, beet.u8)],
    ],
    "MessageAddressTableLookup"
  );

export type TransactionMessage = {
  numSigners: number;
  numWritableSigners: number;
  numWritableNonSigners: number;
  accountKeys: PublicKey[];
  instructions: CompiledMsInstruction[];
  addressTableLookups: MessageAddressTableLookup[];
};

export const transactionMessageBeet =
  new beet.FixableBeetArgsStruct<TransactionMessage>(
    [
      ["numSigners", beet.u8],
      ["numWritableSigners", beet.u8],
      ["numWritableNonSigners", beet.u8],
      ["accountKeys", smallArray(beet.u8, beetSolana.publicKey)],
      ["instructions", smallArray(beet.u8, compiledMsInstructionBeet)],
      [
        "addressTableLookups",
        smallArray(beet.u8, messageAddressTableLookupBeet),
      ],
    ],
    "TransactionMessage"
  );

/**
 * Custom beet serializer for SmartAccountSignerWrapper that matches Rust's custom Borsh format.
 *
 * Users pass plain arrays of signers:
 * - V1: LegacySmartAccountSigner[] (no __kind field)
 * - V2: SmartAccountSigner[] (with __kind: "Native" | "SessionKey" | etc)
 *
 * The SDK auto-wraps with version bytes:
 * - V1 format: [count_low, count_high, 0, 0] + LegacySmartAccountSigner[]
 * - V2 format: [count_low, count_high, 0, 1] + SmartAccountSigner[]
 *
 * The SmartAccountSignerWrapper enum only exists in Rust for versioning.
 */
import {
  SmartAccountSignerWrapper,
  SmartAccountSigner,
  LegacySmartAccountSigner,
  smartAccountSignerBeet,
  legacySmartAccountSignerBeet,
} from "./generated";

/**
 * Pack a SmartAccountSigner into the custom packed format that Rust expects.
 * Returns { tag, payload } where payload is the packed bytes.
 */
function packSigner(signer: SmartAccountSigner): { tag: number; payload: Buffer } {
  const buf = Buffer.alloc(256); // Max size buffer
  let cursor = 0;

  if (signer.__kind === "Native") {
    // Native: [32 byte key][1 byte permissions]
    signer.key.toBuffer().copy(buf, cursor);
    cursor += 32;
    buf.writeUInt8(signer.permissions.mask, cursor);
    cursor += 1;
    return { tag: 0, payload: buf.slice(0, cursor) };
  } else if (signer.__kind === "Ed25519External") {
    // Ed25519External: [1 byte permissions][32 byte external_pubkey][32 byte session_key][8 bytes expiration][8 bytes nonce]
    buf.writeUInt8(signer.permissions.mask, cursor);
    cursor += 1;
    Buffer.from(signer.data.externalPubkey).copy(buf, cursor);
    cursor += 32;
    signer.data.sessionKeyData.key.toBuffer().copy(buf, cursor);
    cursor += 32;
    buf.writeBigUInt64LE(BigInt(typeof signer.data.sessionKeyData.expiration === 'number' ? signer.data.sessionKeyData.expiration : signer.data.sessionKeyData.expiration.toNumber()), cursor);
    cursor += 8;
    buf.writeBigUInt64LE(BigInt(typeof signer.nonce === 'number' ? signer.nonce : signer.nonce.toNumber()), cursor);
    cursor += 8;
    return { tag: 3, payload: buf.slice(0, cursor) };
  } else if (signer.__kind === "Secp256k1") {
    // Secp256k1: [1 byte permissions][64 byte uncompressed_pubkey][20 byte eth_address][1 byte has_eth_address][32 byte session_key][8 bytes expiration][8 bytes nonce]
    buf.writeUInt8(signer.permissions.mask, cursor);
    cursor += 1;
    Buffer.from(signer.data.uncompressedPubkey).copy(buf, cursor);
    cursor += 64;
    Buffer.from(signer.data.ethAddress).copy(buf, cursor);
    cursor += 20;
    buf.writeUInt8(signer.data.hasEthAddress ? 1 : 0, cursor);
    cursor += 1;
    signer.data.sessionKeyData.key.toBuffer().copy(buf, cursor);
    cursor += 32;
    buf.writeBigUInt64LE(BigInt(typeof signer.data.sessionKeyData.expiration === 'number' ? signer.data.sessionKeyData.expiration : signer.data.sessionKeyData.expiration.toNumber()), cursor);
    cursor += 8;
    buf.writeBigUInt64LE(BigInt(typeof signer.nonce === 'number' ? signer.nonce : signer.nonce.toNumber()), cursor);
    cursor += 8;
    return { tag: 2, payload: buf.slice(0, cursor) };
  } else if (signer.__kind === "P256Webauthn") {
    // P256Webauthn: [1 byte permissions][33 byte compressed_pubkey][1 byte rp_id_len][32 byte rp_id][32 byte rp_id_hash][8 bytes counter][32 byte session_key][8 bytes expiration][8 bytes nonce]
    buf.writeUInt8(signer.permissions.mask, cursor);
    cursor += 1;
    Buffer.from(signer.data.compressedPubkey).copy(buf, cursor);
    cursor += 33;
    buf.writeUInt8(signer.data.rpIdLen, cursor);
    cursor += 1;
    Buffer.from(signer.data.rpId).copy(buf, cursor);
    cursor += 32;
    Buffer.from(signer.data.rpIdHash).copy(buf, cursor);
    cursor += 32;
    buf.writeBigUInt64LE(BigInt(typeof signer.data.counter === 'number' ? signer.data.counter : signer.data.counter.toNumber()), cursor);
    cursor += 8;
    signer.data.sessionKeyData.key.toBuffer().copy(buf, cursor);
    cursor += 32;
    buf.writeBigUInt64LE(BigInt(typeof signer.data.sessionKeyData.expiration === 'number' ? signer.data.sessionKeyData.expiration : signer.data.sessionKeyData.expiration.toNumber()), cursor);
    cursor += 8;
    buf.writeBigUInt64LE(BigInt(typeof signer.nonce === 'number' ? signer.nonce : signer.nonce.toNumber()), cursor);
    cursor += 8;
    return { tag: 1, payload: buf.slice(0, cursor) };
  } else if (signer.__kind === "P256Native") {
    // P256Native: [1 byte permissions][33 byte compressed_pubkey][32 byte session_key][8 bytes expiration][8 bytes nonce]
    buf.writeUInt8(signer.permissions.mask, cursor);
    cursor += 1;
    Buffer.from(signer.data.compressedPubkey).copy(buf, cursor);
    cursor += 33;
    signer.data.sessionKeyData.key.toBuffer().copy(buf, cursor);
    cursor += 32;
    buf.writeBigUInt64LE(BigInt(typeof signer.data.sessionKeyData.expiration === 'number' ? signer.data.sessionKeyData.expiration : signer.data.sessionKeyData.expiration.toNumber()), cursor);
    cursor += 8;
    buf.writeBigUInt64LE(BigInt(typeof signer.nonce === 'number' ? signer.nonce : signer.nonce.toNumber()), cursor);
    cursor += 8;
    return { tag: 4, payload: buf.slice(0, cursor) };
  }

  throw new Error(`Unknown signer type: ${(signer as any).__kind}`);
}

/**
 * Unpack a SmartAccountSigner from the custom packed format.
 */
function unpackSigner(tag: number, payload: Buffer): SmartAccountSigner {
  let cursor = 0;

  if (tag === 0) {
    // Native
    const key = new PublicKey(payload.slice(cursor, cursor + 32));
    cursor += 32;
    const permissions = { mask: payload.readUInt8(cursor) };
    return { __kind: "Native", key, permissions };
  } else if (tag === 3) {
    // Ed25519External
    const permissions = { mask: payload.readUInt8(cursor) };
    cursor += 1;
    const externalPubkey = Array.from(payload.slice(cursor, cursor + 32));
    cursor += 32;
    const sessionKey = new PublicKey(payload.slice(cursor, cursor + 32));
    cursor += 32;
    const expiration = Number(payload.readBigUInt64LE(cursor));
    cursor += 8;
    const nonce = Number(payload.readBigUInt64LE(cursor));
    return {
      __kind: "Ed25519External",
      permissions,
      data: {
        externalPubkey,
        sessionKeyData: { key: sessionKey, expiration },
      },
      nonce,
    };
  } else if (tag === 2) {
    // Secp256k1
    const permissions = { mask: payload.readUInt8(cursor) };
    cursor += 1;
    const uncompressedPubkey = Array.from(payload.slice(cursor, cursor + 64));
    cursor += 64;
    const ethAddress = Array.from(payload.slice(cursor, cursor + 20));
    cursor += 20;
    const hasEthAddress = payload.readUInt8(cursor) === 1;
    cursor += 1;
    const sessionKey = new PublicKey(payload.slice(cursor, cursor + 32));
    cursor += 32;
    const expiration = Number(payload.readBigUInt64LE(cursor));
    cursor += 8;
    const nonce = Number(payload.readBigUInt64LE(cursor));
    return {
      __kind: "Secp256k1",
      permissions,
      data: {
        uncompressedPubkey,
        ethAddress,
        hasEthAddress,
        sessionKeyData: { key: sessionKey, expiration },
      },
      nonce,
    };
  } else if (tag === 1) {
    // P256Webauthn
    const permissions = { mask: payload.readUInt8(cursor) };
    cursor += 1;
    const compressedPubkey = Array.from(payload.slice(cursor, cursor + 33));
    cursor += 33;
    const rpIdLen = payload.readUInt8(cursor);
    cursor += 1;
    const rpId = Array.from(payload.slice(cursor, cursor + 32));
    cursor += 32;
    const rpIdHash = Array.from(payload.slice(cursor, cursor + 32));
    cursor += 32;
    const counter = Number(payload.readBigUInt64LE(cursor));
    cursor += 8;
    const sessionKey = new PublicKey(payload.slice(cursor, cursor + 32));
    cursor += 32;
    const expiration = Number(payload.readBigUInt64LE(cursor));
    cursor += 8;
    const nonce = Number(payload.readBigUInt64LE(cursor));
    return {
      __kind: "P256Webauthn",
      permissions,
      data: {
        compressedPubkey,
        rpIdLen,
        rpId,
        rpIdHash,
        counter,
        sessionKeyData: { key: sessionKey, expiration },
      },
      nonce,
    };
  } else if (tag === 4) {
    // P256Native
    const permissions = { mask: payload.readUInt8(cursor) };
    cursor += 1;
    const compressedPubkey = Array.from(payload.slice(cursor, cursor + 33));
    cursor += 33;
    const sessionKey = new PublicKey(payload.slice(cursor, cursor + 32));
    cursor += 32;
    const expiration = Number(payload.readBigUInt64LE(cursor));
    cursor += 8;
    const nonce = Number(payload.readBigUInt64LE(cursor));
    return {
      __kind: "P256Native",
      permissions,
      data: {
        compressedPubkey,
        sessionKeyData: { key: sessionKey, expiration },
      },
      nonce,
    };
  }

  throw new Error(`Unknown signer tag: ${tag}`);
}

export const customSmartAccountSignerWrapperBeet = {
  toFixedFromData(buf: Buffer, offset: number): beet.FixedSizeBeet<LegacySmartAccountSigner[] | SmartAccountSigner[]> {
    const countLow = buf.readUInt8(offset);
    const countHigh = buf.readUInt8(offset + 1);
    const count = countLow | (countHigh << 8);
    const version = buf.readUInt8(offset + 3);

    let cursor = offset + 4;

    if (version === 0) {
      // V1: LegacySmartAccountSigner[]
      const signers: beet.FixedSizeBeet<LegacySmartAccountSigner>[] = [];
      for (let i = 0; i < count; i++) {
        const fixedSigner = beet.fixBeetFromData(legacySmartAccountSignerBeet, buf, cursor);
        signers.push(fixedSigner);
        cursor += fixedSigner.byteSize;
      }
      const signersSize = cursor - (offset + 4);

      return {
        write(buf: Buffer, offset: number, value: any): void {
          buf.writeUInt8(value.length & 0xff, offset);
          buf.writeUInt8((value.length >> 8) & 0xff, offset + 1);
          buf.writeUInt8(0, offset + 2);
          buf.writeUInt8(0, offset + 3);
          let cursor = offset + 4;
          for (let i = 0; i < value.length; i++) {
            signers[i].write(buf, cursor, value[i] as any);
            cursor += signers[i].byteSize;
          }
        },
        read(buf: Buffer, offset: number): LegacySmartAccountSigner[] {
          const signersArray: LegacySmartAccountSigner[] = [];
          let cursor = offset + 4;
          for (const signer of signers) {
            signersArray.push(signer.read(buf, cursor));
            cursor += signer.byteSize;
          }
          return signersArray;
        },
        byteSize: 4 + signersSize,
        description: 'SmartAccountSignerWrapper::V1',
      };
    } else {
      // V2: SmartAccountSigner[] - Read packed format
      const ENTRY_HEADER_LEN = 4;
      const packedSigners: { tag: number; payload: Buffer }[] = [];
      let signersSize = 0;

      // Read each packed signer from buffer
      for (let i = 0; i < count; i++) {
        const tag = buf.readUInt8(cursor);
        const payloadLen = buf.readUInt16LE(cursor + 1);
        cursor += ENTRY_HEADER_LEN;

        const payload = buf.slice(cursor, cursor + payloadLen);
        packedSigners.push({ tag, payload });
        signersSize += ENTRY_HEADER_LEN + payloadLen;
        cursor += payloadLen;
      }

      return {
        write(buf: Buffer, offset: number, value: any): void {
          // Write V2 format: [count_low, count_high, 0, 1] + packed signers
          buf.writeUInt8(value.length & 0xff, offset);
          buf.writeUInt8((value.length >> 8) & 0xff, offset + 1);
          buf.writeUInt8(0, offset + 2);
          buf.writeUInt8(1, offset + 3); // Version byte = 1

          let cursor = offset + 4;
          for (const signer of value) {
            const packed = packSigner(signer);
            // Write per-signer header: [tag, len_low, len_high, flags]
            buf.writeUInt8(packed.tag, cursor);
            buf.writeUInt16LE(packed.payload.length, cursor + 1);
            buf.writeUInt8(0, cursor + 3); // flags (reserved)
            cursor += ENTRY_HEADER_LEN;

            // Write packed payload
            packed.payload.copy(buf, cursor);
            cursor += packed.payload.length;
          }
        },
        read(buf: Buffer, offset: number): SmartAccountSigner[] {
          const signersArray: SmartAccountSigner[] = [];
          let cursor = offset + 4;

          for (const packed of packedSigners) {
            const signer = unpackSigner(packed.tag, packed.payload);
            signersArray.push(signer);
          }

          return signersArray;
        },
        byteSize: 4 + signersSize,
        description: 'SmartAccountSignerWrapper::V2 (packed)',
      };
    }
  },

  toFixedFromValue(value: LegacySmartAccountSigner[] | SmartAccountSigner[]): beet.FixedSizeBeet<LegacySmartAccountSigner[] | SmartAccountSigner[]> {
    // Simple detection: V2 if first item has __kind, otherwise V1
    const signers = value;
    const isV2 = signers.length > 0 && '__kind' in signers[0];

    if (!isV2) {
      // V1: LegacySmartAccountSigner[] (no __kind field)
      const fixedSigners: beet.FixedSizeBeet<LegacySmartAccountSigner>[] = [];
      let signersSize = 0;
      for (const signer of signers) {
        const fixedSigner = beet.fixBeetFromValue(legacySmartAccountSignerBeet, signer as any);
        fixedSigners.push(fixedSigner);
        signersSize += fixedSigner.byteSize;
      }

      return {
        write(buf: Buffer, offset: number, value: any): void {
          // Write V1 format: [count_low, count_high, 0, 0] + LegacySmartAccountSigner[]
          buf.writeUInt8(value.length & 0xff, offset);
          buf.writeUInt8((value.length >> 8) & 0xff, offset + 1);
          buf.writeUInt8(0, offset + 2);
          buf.writeUInt8(0, offset + 3); // Version byte = 0
          let cursor = offset + 4;
          for (let i = 0; i < value.length; i++) {
            fixedSigners[i].write(buf, cursor, value[i] as any);
            cursor += fixedSigners[i].byteSize;
          }
        },
        read(buf: Buffer, offset: number): LegacySmartAccountSigner[] {
          const signersArray: LegacySmartAccountSigner[] = [];
          let cursor = offset + 4;
          for (const signer of fixedSigners) {
            signersArray.push(signer.read(buf, cursor));
            cursor += signer.byteSize;
          }
          return signersArray;
        },
        byteSize: 4 + signersSize,
        description: 'SmartAccountSignerWrapper::V1',
      };
    } else {
      // V2: SmartAccountSigner[] (with __kind field) - use packed format
      // Calculate total size with packed format: 4-byte wrapper header + per-signer (4-byte header + payload)
      const ENTRY_HEADER_LEN = 4;
      let signersSize = 0;
      const packedSigners: { tag: number; payload: Buffer }[] = [];

      for (const signer of signers) {
        const packed = packSigner(signer as any);
        packedSigners.push(packed);
        signersSize += ENTRY_HEADER_LEN + packed.payload.length;
      }

      return {
        write(buf: Buffer, offset: number, value: any): void {
          // Write V2 format: [count_low, count_high, 0, 1] + packed signers
          buf.writeUInt8(value.length & 0xff, offset);
          buf.writeUInt8((value.length >> 8) & 0xff, offset + 1);
          buf.writeUInt8(0, offset + 2);
          buf.writeUInt8(1, offset + 3); // Version byte = 1

          let cursor = offset + 4;
          for (const packed of packedSigners) {
            // Write per-signer header: [tag, len_low, len_high, flags]
            buf.writeUInt8(packed.tag, cursor);
            buf.writeUInt16LE(packed.payload.length, cursor + 1);
            buf.writeUInt8(0, cursor + 3); // flags (reserved)
            cursor += ENTRY_HEADER_LEN;

            // Write packed payload
            packed.payload.copy(buf, cursor);
            cursor += packed.payload.length;
          }
        },
        read(buf: Buffer, offset: number): SmartAccountSigner[] {
          const signersArray: SmartAccountSigner[] = [];
          let cursor = offset + 4;

          for (let i = 0; i < value.length; i++) {
            // Read per-signer header
            const tag = buf.readUInt8(cursor);
            const payloadLen = buf.readUInt16LE(cursor + 1);
            cursor += ENTRY_HEADER_LEN;

            // Read and unpack payload
            const payload = buf.slice(cursor, cursor + payloadLen);
            const signer = unpackSigner(tag, payload);
            signersArray.push(signer);
            cursor += payloadLen;
          }

          return signersArray;
        },
        byteSize: 4 + signersSize,
        description: 'SmartAccountSignerWrapper::V2 (packed)',
      };
    }
  },

  description: 'SmartAccountSignerWrapper (custom)',
} as beet.FixableBeet<LegacySmartAccountSigner[] | SmartAccountSigner[], LegacySmartAccountSigner[] | SmartAccountSigner[]>;

