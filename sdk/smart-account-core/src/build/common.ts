import { type AccountMeta, AccountRole, type AccountSignerMeta, type Address } from '@solana/kit';
import { SmartAccountError } from '../errors.ts';
import { maskOf } from '../encoding/accounts.ts';
import type { AccountSnapshot, SignerEntry, SignerInput } from '../types.ts';

export const ALL_PERMISSIONS = 7;
const VOTE = 2;

export const keyOf = (s: SignerInput): Address => (typeof s === 'string' ? s : s.address);

export const meta = (address: Address, role: AccountRole): AccountMeta => ({ address, role });

export const signerMeta = (s: SignerInput, writable = false): AccountMeta | AccountSignerMeta => {
  const role = writable ? AccountRole.WRITABLE_SIGNER : AccountRole.READONLY_SIGNER;
  return typeof s === 'string' ? { address: s, role } : { address: s.address, role, signer: s };
};

export function requireSync(account: AccountSnapshot): void {
  if (account.timeLock !== 0) {
    throw new SmartAccountError('SYNC_NOT_ALLOWED', `account has a time lock of ${account.timeLock}s`);
  }
}

export function requireAutonomous(account: AccountSnapshot): void {
  if (account.settingsAuthority !== null) {
    throw new SmartAccountError('SYNC_NOT_ALLOWED', `account is controlled by settings authority ${account.settingsAuthority}`);
  }
}

/** The transaction signers must be distinct members whose votes reach the threshold and who together hold all permissions. */
export function checkTransactionSigners(account: AccountSnapshot, signers: readonly SignerInput[]): void {
  const fail = (msg: string) => new SmartAccountError('INSUFFICIENT_SIGNERS', msg);
  if (signers.length === 0) throw fail('no signers given');
  if (signers.length > 0xff) throw fail('more than 255 signers');
  let union = 0;
  let voters = 0;
  const seen = new Set<Address>();
  for (const s of signers) {
    const key = keyOf(s);
    if (seen.has(key)) throw fail(`duplicate signer ${key}`);
    seen.add(key);
    const member = account.signers.find((m) => m.key === key);
    if (!member) throw fail(`${key} is not a signer of ${account.settings}`);
    const mask = maskOf(member.permissions) ?? 0;
    union |= mask;
    if (mask & VOTE) voters++;
  }
  if (voters < account.threshold) throw fail(`${voters} voters given, threshold is ${account.threshold}`);
  if (union !== ALL_PERMISSIONS) throw fail('signers do not hold initiate, vote and execute between them');
}

/** Validates a proposed signer set and threshold; returns each signer's permission mask. */
export function checkSignerSet(signers: readonly SignerEntry[], threshold: number): number[] {
  const fail = (msg: string) => new SmartAccountError('INVALID_SIGNER_SET', msg);
  if (signers.length === 0) throw fail('signer set is empty');
  if (signers.length > 0xffff) throw fail('more than 65535 signers');
  const keys = new Set(signers.map((s) => s.key));
  if (keys.size !== signers.length) throw fail('signer set has duplicate keys');
  const masks = signers.map((s) => {
    const m = maskOf(s.permissions);
    if (m === null) throw fail(`${s.key} has an unknown permission`);
    return m;
  });
  const voters = masks.filter((m) => m & VOTE).length;
  if (masks.reduce((a, m) => a | m, 0) !== ALL_PERMISSIONS) throw fail('signers must hold initiate, vote and execute between them');
  if (!Number.isInteger(threshold) || threshold < 1 || threshold > voters) {
    throw fail(`threshold ${threshold} must be between 1 and the number of voters (${voters})`);
  }
  return masks;
}
