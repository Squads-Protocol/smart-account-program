import {
  type Address,
  type Base64EncodedBytes,
  getAddressDecoder,
  getAddressEncoder,
  getBase64Decoder,
  getBase64Encoder,
  type ReadonlyUint8Array,
} from '@solana/kit';
import { SmartAccountError } from '../errors.ts';

const addressEncoder = getAddressEncoder();
const addressDecoder = getAddressDecoder();

export const concat = (...parts: readonly ReadonlyUint8Array[]): Uint8Array => {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let offset = 0;
  for (const p of parts) {
    out.set(p, offset);
    offset += p.length;
  }
  return out;
};

const fixed = (size: number, write: (v: DataView) => void): Uint8Array => {
  const b = new Uint8Array(size);
  write(new DataView(b.buffer));
  return b;
};

const checkInt = (n: number, max: number, what: string): number => {
  if (!Number.isInteger(n) || n < 0 || n > max) {
    throw new SmartAccountError('INVALID_ARGUMENT', `${what} must be an integer in [0, ${max}], got ${n}`);
  }
  return n;
};

const checkBig = (n: bigint, bits: bigint, what: string): bigint => {
  if (n < 0n || n >= 1n << bits) throw new SmartAccountError('INVALID_ARGUMENT', `${what} out of range for u${bits}`);
  return n;
};

export const u8 = (n: number, what = 'u8'): Uint8Array => Uint8Array.of(checkInt(n, 0xff, what));
export const u16 = (n: number, what = 'u16'): Uint8Array =>
  fixed(2, (v) => v.setUint16(0, checkInt(n, 0xffff, what), true));
export const u32 = (n: number, what = 'u32'): Uint8Array =>
  fixed(4, (v) => v.setUint32(0, checkInt(n, 0xffff_ffff, what), true));
export const u64 = (n: bigint, what = 'u64'): Uint8Array =>
  fixed(8, (v) => v.setBigUint64(0, checkBig(n, 64n, what), true));
export const u128 = (n: bigint, what = 'u128'): Uint8Array => {
  checkBig(n, 128n, what);
  return concat(u64(n & 0xffff_ffff_ffff_ffffn), u64(n >> 64n));
};

export const pubkey = (a: Address): Uint8Array => new Uint8Array(addressEncoder.encode(a));
export const vec = (items: readonly Uint8Array[]): Uint8Array => concat(u32(items.length), ...items);
export const bytesVec = (b: Uint8Array): Uint8Array => concat(u32(b.length), b);
export const option = <T>(v: T | null | undefined, enc: (v: T) => Uint8Array): Uint8Array =>
  v === null || v === undefined ? Uint8Array.of(0) : concat(Uint8Array.of(1), enc(v));
export const string = (s: string): Uint8Array => bytesVec(new TextEncoder().encode(s));

export const fromHex = (s: string): Uint8Array => Uint8Array.from(s.match(/../g) ?? [], (h) => parseInt(h, 16));

export const fromBase64 = (s: string): Uint8Array => new Uint8Array(getBase64Encoder().encode(s));
export const toBase64Bytes = (b: Uint8Array): Base64EncodedBytes => getBase64Decoder().decode(b) as Base64EncodedBytes;

export const compareBytes = (a: Uint8Array, b: Uint8Array): number => {
  for (let i = 0; i < Math.min(a.length, b.length); i++) {
    const d = (a[i] ?? 0) - (b[i] ?? 0);
    if (d !== 0) return d;
  }
  return a.length - b.length;
};

export interface Reader {
  readonly offset: () => number;
  readonly u8: () => number;
  readonly u16: () => number;
  readonly u32: () => number;
  readonly u64: () => bigint;
  readonly i64: () => bigint;
  readonly u128: () => bigint;
  readonly pubkey: () => Address;
  readonly option: <T>(read: () => T) => T | null;
  readonly vec: <T>(read: () => T) => T[];
  readonly bytes: (n: number) => Uint8Array;
}

/** Bounds-checked Borsh reader; throws RangeError past the end of `data`. */
export function reader(data: Uint8Array, start = 0): Reader {
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  let o = start;
  const take = (n: number): number => {
    if (o + n > data.length) throw new RangeError(`read past end (${o + n} > ${data.length})`);
    const at = o;
    o += n;
    return at;
  };
  const r: Reader = {
    offset: () => o,
    u8: () => view.getUint8(take(1)),
    u16: () => view.getUint16(take(2), true),
    u32: () => view.getUint32(take(4), true),
    u64: () => view.getBigUint64(take(8), true),
    i64: () => view.getBigInt64(take(8), true),
    u128: () => {
      const lo = r.u64();
      return lo | (r.u64() << 64n);
    },
    pubkey: () => addressDecoder.decode(data.subarray(take(32), o)),
    option: (read) => {
      const tag = r.u8();
      if (tag > 1) throw new RangeError(`invalid Option tag ${tag}`);
      return tag === 1 ? read() : null;
    },
    vec: (read) => {
      const n = r.u32();
      if (n > data.length - o) throw new RangeError(`vec length ${n} exceeds remaining data`);
      return Array.from({ length: n }, read);
    },
    bytes: (n) => data.slice(take(n), o),
  };
  return r;
}
