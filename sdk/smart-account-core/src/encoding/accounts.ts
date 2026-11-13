import type { Address } from '@solana/kit';
import type { Permission, ProgramConfig, SignerEntry, SpendingLimitPeriod } from '../types.ts';
import { reader } from './bytes.ts';
import { DISC, SYSTEM_PROGRAM_ADDRESS } from './constants.ts';

const PERMISSION_BITS: readonly (readonly [Permission, number])[] = [
  ['initiate', 1],
  ['vote', 2],
  ['execute', 4],
];

/** Returns null for an unknown permission name. */
export function maskOf(permissions: readonly string[]): number | null {
  let mask = 0;
  for (const p of permissions) {
    const bit = PERMISSION_BITS.find(([name]) => name === p)?.[1];
    if (bit === undefined) return null;
    mask |= bit;
  }
  return mask;
}

export const permissionsOf = (mask: number): Permission[] =>
  PERMISSION_BITS.filter(([, bit]) => (mask & bit) !== 0).map(([name]) => name);

export const hasDiscriminator = (data: Uint8Array, disc: Uint8Array): boolean =>
  data.length >= 8 && disc.every((b, i) => data[i] === b);

export interface DecodedSettings {
  seed: bigint;
  settingsAuthority: Address | null;
  threshold: number;
  timeLock: number;
  transactionIndex: bigint;
  staleTransactionIndex: bigint;
  archivalAuthority: Address | null;
  archivableAfter: bigint;
  bump: number;
  signers: SignerEntry[];
  signerMasks: number[];
  accountUtilization: number;
  policySeed: bigint | null;
  reserved2: number;
}

/** Mainnet layout: `policy_seed: Option<u64>` follows `account_utilization` (the fork IDL has `reserved1: u8`). */
export function decodeSettings(data: Uint8Array): DecodedSettings {
  if (!hasDiscriminator(data, DISC.settings)) throw new RangeError('not a Settings account');
  const r = reader(data, 8);
  const seed = r.u128();
  const authority = r.pubkey();
  const threshold = r.u16();
  const timeLock = r.u32();
  const transactionIndex = r.u64();
  const staleTransactionIndex = r.u64();
  const archivalAuthority = r.option(r.pubkey);
  const archivableAfter = r.u64();
  const bump = r.u8();
  const raw = r.vec(() => {
    const key = r.pubkey();
    const mask = r.u8();
    return { key, mask };
  });
  const accountUtilization = r.u8();
  const policySeed = r.option(r.u64);
  const reserved2 = r.u8();
  return {
    seed,
    settingsAuthority: authority === SYSTEM_PROGRAM_ADDRESS ? null : authority,
    threshold,
    timeLock,
    transactionIndex,
    staleTransactionIndex,
    archivalAuthority,
    archivableAfter,
    bump,
    signers: raw.map((s) => ({ key: s.key, permissions: permissionsOf(s.mask) })),
    signerMasks: raw.map((s) => s.mask),
    accountUtilization,
    policySeed,
    reserved2,
  };
}

const PERIODS: readonly SpendingLimitPeriod[] = ['oneTime', 'day', 'week', 'month'];

export function decodeSpendingLimit(data: Uint8Array) {
  if (!hasDiscriminator(data, DISC.spendingLimit)) throw new RangeError('not a SpendingLimit account');
  const r = reader(data, 8);
  const settings = r.pubkey();
  const seed = r.pubkey();
  const accountIndex = r.u8();
  const mint = r.pubkey();
  const amount = r.u64();
  const periodIndex = r.u8();
  const period = PERIODS[periodIndex];
  if (period === undefined) throw new RangeError(`unknown spending limit period ${periodIndex}`);
  const remainingAmount = r.u64();
  const lastReset = r.i64();
  const bump = r.u8();
  const signers = r.vec(r.pubkey);
  const destinations = r.vec(r.pubkey);
  const expiration = r.i64();
  return { settings, seed, accountIndex, mint, amount, period, remainingAmount, lastReset, bump, signers, destinations, expiration };
}

export function decodeProgramConfig(data: Uint8Array): ProgramConfig {
  if (!hasDiscriminator(data, DISC.programConfig)) throw new RangeError('not a ProgramConfig account');
  const r = reader(data, 8);
  const smartAccountIndex = r.u128();
  const authority = r.pubkey();
  const smartAccountCreationFee = r.u64();
  const treasury = r.pubkey();
  return { smartAccountIndex, authority, smartAccountCreationFee, treasury };
}

/** SPL Token / Token-2022 account base layout (first 165 bytes are shared). */
export function decodeTokenAccount(data: Uint8Array): { mint: Address; owner: Address; amount: bigint } {
  if (data.length < 165) throw new RangeError('token account too short');
  const r = reader(data);
  const mint = r.pubkey();
  const owner = r.pubkey();
  const amount = r.u64();
  return { mint, owner, amount };
}

/** Mint layout: COption<Pubkey> authority (36), supply u64 (8), decimals u8 at offset 44. */
export function decodeMintDecimals(data: Uint8Array): number {
  if (data.length < 82) throw new RangeError('mint account too short');
  return reader(data, 44).u8();
}
