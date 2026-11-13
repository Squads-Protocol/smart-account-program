import assert from 'node:assert/strict';

import {
  AccountRole,
  type Address,
  type Instruction,
  type TransactionSigner,
  createNoopSigner,
  isSignerRole,
  isWritableRole,
} from '@solana/kit';

import { type ErrorCode, type Permission, type SignerEntry, sdk } from './sdk.ts';

export const hex = (b: Uint8Array | ArrayLike<number>): string => Buffer.from(Uint8Array.from(b)).toString('hex');
export const unhex = (h: string): Uint8Array => new Uint8Array(Buffer.from(h, 'hex'));

/** JSON.stringify that survives bigints (for messages and loose content checks). */
export const show = (v: unknown): string =>
  JSON.stringify(v, (_k, x: unknown) => (typeof x === 'bigint' ? `${x}n` : x instanceof Uint8Array ? hex(x) : x));

// ---------- permissions ----------
export const PERMISSIONS: readonly Permission[] = ['initiate', 'vote', 'execute'];
export const ALL: readonly Permission[] = PERMISSIONS;
export const maskToPerms = (mask: number): Permission[] => PERMISSIONS.filter((_, i) => (mask >> i) & 1);
export const permsToMask = (p: readonly Permission[]): number =>
  p.reduce((m, x) => m | (1 << PERMISSIONS.indexOf(x)), 0);
export const entry = (key: Address, mask: number): SignerEntry => ({ key, permissions: maskToPerms(mask) });
/** Signer set as a canonical, order-free description: "key:mask" sorted. */
export const signerSetKey = (s: readonly { key: Address; mask: number }[]): string[] =>
  s.map((x) => `${x.key}:${x.mask}`).sort();
export const entriesToMasks = (s: readonly SignerEntry[]) => s.map((x) => ({ key: x.key, mask: permsToMask(x.permissions) }));

// ---------- instructions ----------
export interface PlainIx {
  programId: string;
  accounts: { pubkey: string; isSigner: boolean; isWritable: boolean }[];
  dataHex: string;
}
export const roleOf = (isSigner: boolean, isWritable: boolean): AccountRole =>
  isSigner
    ? isWritable
      ? AccountRole.WRITABLE_SIGNER
      : AccountRole.READONLY_SIGNER
    : isWritable
      ? AccountRole.WRITABLE
      : AccountRole.READONLY;
/** SDK (kit) instruction -> the vector's plain shape. Embedded signer objects are ignored. */
export const plain = (ix: Instruction): PlainIx => ({
  programId: ix.programAddress,
  accounts: (ix.accounts ?? []).map((a) => ({
    pubkey: a.address,
    isSigner: isSignerRole(a.role),
    isWritable: isWritableRole(a.role),
  })),
  dataHex: hex(ix.data ?? new Uint8Array()),
});
/** Vector plain instruction -> kit instruction (input side). */
export const kitIx = (p: PlainIx): Instruction => ({
  programAddress: p.programId as Address,
  accounts: p.accounts.map((a) => ({ address: a.pubkey as Address, role: roleOf(a.isSigner, a.isWritable) })),
  data: unhex(p.dataHex),
});
export const noop = (a: string): TransactionSigner => createNoopSigner(a as Address);
export const dataOf = (ix: Instruction): Uint8Array => Uint8Array.from(ix.data ?? new Uint8Array());

// ---------- outcomes ----------
export type Settled<T> = { ok: true; value: T } | { ok: false; error: unknown };
/** Runs fn, catching both a synchronous throw and a rejected promise. */
export async function settle<T>(fn: () => T | Promise<T>): Promise<Settled<T>> {
  try {
    return { ok: true, value: await fn() };
  } catch (error) {
    return { ok: false, error };
  }
}
export const codeOf = (e: unknown): string | undefined =>
  typeof e === 'object' && e !== null && 'code' in e && typeof e.code === 'string' ? e.code : undefined;

/** Asserts a typed SDK error (extends MigrationSdkError when exported) with one of `codes`. */
export function assertSdkError(e: unknown, codes: ErrorCode | readonly ErrorCode[], label = ''): void {
  const want: readonly string[] = typeof codes === 'string' ? [codes] : codes;
  assert.ok(e instanceof Error, `${label}: expected an Error, got ${show(e)}`);
  if (sdk.MigrationSdkError !== undefined) {
    assert.ok(e instanceof sdk.MigrationSdkError, `${label}: error does not extend MigrationSdkError: ${String(e)}`);
  }
  const code = codeOf(e);
  assert.ok(code !== undefined && want.includes(code), `${label}: expected code ${want.join('|')}, got ${code} (${String(e)})`);
}
export async function rejectsWith(
  fn: () => unknown,
  codes: ErrorCode | readonly ErrorCode[],
  label = '',
): Promise<void> {
  const r = await settle(fn);
  assert.equal(r.ok, false, `${label}: expected a ${String(codes)} error, but the call succeeded`);
  if (!r.ok) assertSdkError(r.error, codes, label);
}
export async function must<T>(fn: () => T | Promise<T>, label = ''): Promise<T> {
  const r = await settle(fn);
  if (!r.ok) assert.fail(`${label}: unexpected error ${codeOf(r.error) ?? ''} ${String(r.error)}`);
  return r.value;
}
