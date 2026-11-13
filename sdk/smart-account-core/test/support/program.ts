// Test-side, hand-written oracle for the Smart Account Program wire formats (mainnet layout, see
// test/fixtures/mainnet/IDL-DIFF.md). Used to set up LiteSVM state and to decode SDK output.
// It is independent of src/ and is itself pinned by the Grid vectors (see vectors.test.ts, "harness").
import { createHash } from 'node:crypto';

import {
  AccountRole,
  type AccountMeta,
  type AccountSignerMeta,
  type Address,
  type Instruction,
  type TransactionSigner,
  address,
  getAddressDecoder,
  getAddressEncoder,
  getProgramDerivedAddress,
} from '@solana/kit';

export const PROGRAM_ID = address('SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG');
export const SYSTEM_PROGRAM = address('11111111111111111111111111111111');
export const COMPUTE_BUDGET_PROGRAM = address('ComputeBudget111111111111111111111111111111');
export const TOKEN_PROGRAM = address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
export const TOKEN_2022_PROGRAM = address('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
export const ATA_PROGRAM = address('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
export const DEFAULT_PUBKEY = SYSTEM_PROGRAM; // 32 zero bytes

const addrEnc = getAddressEncoder();
const addrDec = getAddressDecoder();

// ---------- writer ----------
export const concat = (...parts: Uint8Array[]): Uint8Array => {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
};
const le = (n: bigint, len: number) => {
  const b = new Uint8Array(len);
  let v = BigInt.asUintN(len * 8, n);
  for (let i = 0; i < len; i++) {
    b[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return b;
};
export const u8 = (n: number) => Uint8Array.of(n);
export const u16 = (n: number) => le(BigInt(n), 2);
export const u32 = (n: number) => le(BigInt(n), 4);
export const u64 = (n: bigint) => le(n, 8);
export const i64 = (n: bigint) => le(n, 8);
export const u128 = (n: bigint) => le(n, 16);
export const pubkey = (a: Address) => new Uint8Array(addrEnc.encode(a));
export const vec = (items: Uint8Array[]) => concat(u32(items.length), ...items);
export const option = (v: Uint8Array | null) => (v === null ? u8(0) : concat(u8(1), v));
export const str = (s: string) => {
  const b = new TextEncoder().encode(s);
  return concat(u32(b.length), b);
};
export const anchorDisc = (name: string): Uint8Array =>
  new Uint8Array(createHash('sha256').update(`global:${name}`).digest().subarray(0, 8));
export const accountDisc = (name: string): Uint8Array =>
  new Uint8Array(createHash('sha256').update(`account:${name}`).digest().subarray(0, 8));

export const DISC = {
  createSmartAccount: anchorDisc('create_smart_account'),
  executeTransactionSync: anchorDisc('execute_transaction_sync'),
  executeSettingsTransactionSync: anchorDisc('execute_settings_transaction_sync'),
  settings: accountDisc('Settings'),
  spendingLimit: accountDisc('SpendingLimit'),
  programConfig: accountDisc('ProgramConfig'),
};

// ---------- reader ----------
export function reader(data: Uint8Array) {
  let o = 0;
  const take = (n: number) => {
    if (o + n > data.length) throw new Error(`read past end: ${o}+${n} > ${data.length}`);
    const s = data.subarray(o, o + n);
    o += n;
    return s;
  };
  const uint = (n: number) => {
    const b = take(n);
    let v = 0n;
    for (let i = n - 1; i >= 0; i--) v = (v << 8n) | BigInt(b[i]!);
    return v;
  };
  const r = {
    take,
    u8: () => Number(uint(1)),
    u16: () => Number(uint(2)),
    u32: () => Number(uint(4)),
    u64: () => uint(8),
    i64: () => BigInt.asIntN(64, uint(8)),
    u128: () => uint(16),
    pubkey: () => addrDec.decode(take(32)),
    option: <T>(f: () => T): T | null => (r.u8() === 0 ? null : f()),
    vec: <T>(f: () => T): T[] => Array.from({ length: r.u32() }, f),
    str: () => new TextDecoder().decode(take(r.u32())),
    get offset() {
      return o;
    },
    done: () => o === data.length,
  };
  return r;
}

// ---------- PDAs ----------
export const pda = async (programAddress: Address, seeds: (string | Uint8Array)[]) =>
  getProgramDerivedAddress({ programAddress, seeds });
export const programConfigPda = async () => (await pda(PROGRAM_ID, ['smart_account', 'program_config']))[0];
export const settingsPdaWithBump = (index: bigint) => pda(PROGRAM_ID, ['smart_account', 'settings', u128(index)]);
export const settingsPda = async (index: bigint) => (await settingsPdaWithBump(index))[0];
export const vaultPda = async (settings: Address, index: number) =>
  (await pda(PROGRAM_ID, ['smart_account', pubkey(settings), 'smart_account', u8(index)]))[0];
export const spendingLimitPda = async (settings: Address, seed: Address) =>
  (await pda(PROGRAM_ID, ['smart_account', pubkey(settings), 'spending_limit', pubkey(seed)]))[0];
export const ataPda = async (owner: Address, mint: Address, tokenProgram: Address = TOKEN_PROGRAM) =>
  (await pda(ATA_PROGRAM, [pubkey(owner), pubkey(tokenProgram), pubkey(mint)]))[0];

/** Grid's formula (helpers.rs:104-114), as spec §5 buildCreateAccount states it. */
export const creationComputeUnits = (bumps: readonly number[]) =>
  bumps.length === 0 ? 30_000 : 30_000 + (255 - Math.min(...bumps)) * 1_600;

// ---------- instructions (test-only encoders) ----------
type Meta = AccountMeta | AccountSignerMeta;
const signerMeta = (s: TransactionSigner, writable: boolean): AccountSignerMeta => ({
  address: s.address,
  role: writable ? AccountRole.WRITABLE_SIGNER : AccountRole.READONLY_SIGNER,
  signer: s,
});

export function createSmartAccountIx(p: {
  programConfig: Address;
  treasury: Address;
  creator: TransactionSigner;
  signers: readonly { key: Address; mask: number }[];
  threshold: number;
  timeLock?: number;
  settingsAuthority?: Address | null;
  rentCollector?: Address | null;
  memo?: string | null;
  candidates: readonly Address[];
}): Instruction {
  const data = concat(
    DISC.createSmartAccount,
    option(p.settingsAuthority ? pubkey(p.settingsAuthority) : null),
    u16(p.threshold),
    vec(p.signers.map((s) => concat(pubkey(s.key), u8(s.mask)))),
    u32(p.timeLock ?? 0),
    option(p.rentCollector ? pubkey(p.rentCollector) : null),
    option(p.memo != null ? str(p.memo) : null),
  );
  const accounts: Meta[] = [
    { address: p.programConfig, role: AccountRole.WRITABLE },
    { address: p.treasury, role: AccountRole.WRITABLE },
    signerMeta(p.creator, true),
    { address: SYSTEM_PROGRAM, role: AccountRole.READONLY },
    { address: PROGRAM_ID, role: AccountRole.READONLY },
    ...p.candidates.map((a) => ({ address: a, role: AccountRole.WRITABLE })),
  ];
  return { programAddress: PROGRAM_ID, accounts, data };
}

export function addSpendingLimitAction(p: {
  seed: Address;
  accountIndex: number;
  mint: Address;
  amount: bigint;
  period: number; // OneTime 0, Day 1, Week 2, Month 3
  signers: readonly Address[];
  destinations: readonly Address[];
  expiration: bigint;
}): Uint8Array {
  return concat(
    u8(4),
    pubkey(p.seed),
    u8(p.accountIndex),
    pubkey(p.mint),
    u64(p.amount),
    u8(p.period),
    vec(p.signers.map(pubkey)),
    vec(p.destinations.map(pubkey)),
    i64(p.expiration),
  );
}
export const changeThresholdAction = (n: number) => concat(u8(2), u16(n));

/** executeSettingsTransactionSync, test-only (used to add spending limits in LiteSVM). */
export function settingsSyncIx(p: {
  settings: Address;
  rentPayer: TransactionSigner;
  signers: readonly TransactionSigner[];
  actions: readonly Uint8Array[];
  remaining?: readonly Address[];
}): Instruction {
  const data = concat(
    DISC.executeSettingsTransactionSync,
    u8(p.signers.length),
    vec([...p.actions]),
    option(null),
  );
  const accounts: Meta[] = [
    { address: p.settings, role: AccountRole.WRITABLE },
    signerMeta(p.rentPayer, true),
    { address: SYSTEM_PROGRAM, role: AccountRole.READONLY },
    { address: PROGRAM_ID, role: AccountRole.READONLY },
    ...p.signers.map((s) => signerMeta(s, false)),
    ...(p.remaining ?? []).map((a) => ({ address: a, role: AccountRole.WRITABLE })),
  ];
  return { programAddress: PROGRAM_ID, accounts, data };
}

// ---------- decoders ----------
export interface DecodedSettings {
  seed: bigint;
  settingsAuthority: Address;
  threshold: number;
  timeLock: number;
  transactionIndex: bigint;
  staleTransactionIndex: bigint;
  signers: { key: Address; mask: number }[];
}
export function decodeSettingsAccount(data: Uint8Array): DecodedSettings {
  const r = reader(data);
  const disc = r.take(8);
  if (Buffer.compare(disc, DISC.settings) !== 0) throw new Error('not a Settings account');
  const seed = r.u128();
  const settingsAuthority = r.pubkey();
  const threshold = r.u16();
  const timeLock = r.u32();
  const transactionIndex = r.u64();
  const staleTransactionIndex = r.u64();
  r.option(r.pubkey); // archivalAuthority
  r.u64(); // archivableAfter
  r.u8(); // bump
  const signers = r.vec(() => ({ key: r.pubkey(), mask: r.u8() }));
  return { seed, settingsAuthority, threshold, timeLock, transactionIndex, staleTransactionIndex, signers };
}

export type SettingsAction =
  | { type: 'AddSigner'; key: Address; mask: number; raw: Uint8Array }
  | { type: 'RemoveSigner'; key: Address; raw: Uint8Array }
  | { type: 'ChangeThreshold'; threshold: number; raw: Uint8Array }
  | { type: 'SetTimeLock'; timeLock: number; raw: Uint8Array }
  | { type: 'AddSpendingLimit'; raw: Uint8Array }
  | { type: 'RemoveSpendingLimit'; spendingLimit: Address; raw: Uint8Array }
  | { type: 'SetArchivalAuthority'; raw: Uint8Array };

export function decodeSettingsSyncData(data: Uint8Array) {
  const r = reader(data);
  const disc = Buffer.from(r.take(8)).toString('hex');
  const numSigners = r.u8();
  const n = r.u32();
  const actions: SettingsAction[] = [];
  for (let i = 0; i < n; i++) {
    const start = r.offset;
    const tag = r.u8();
    const raw = () => data.slice(start, r.offset);
    switch (tag) {
      case 0: {
        const key = r.pubkey();
        const mask = r.u8();
        actions.push({ type: 'AddSigner', key, mask, raw: raw() });
        break;
      }
      case 1:
        actions.push({ type: 'RemoveSigner', key: r.pubkey(), raw: raw() });
        break;
      case 2:
        actions.push({ type: 'ChangeThreshold', threshold: r.u16(), raw: raw() });
        break;
      case 3:
        actions.push({ type: 'SetTimeLock', timeLock: r.u32(), raw: raw() });
        break;
      case 4:
        r.take(32 + 1 + 32 + 8 + 1);
        r.vec(r.pubkey);
        r.vec(r.pubkey);
        r.i64();
        actions.push({ type: 'AddSpendingLimit', raw: raw() });
        break;
      case 5:
        actions.push({ type: 'RemoveSpendingLimit', spendingLimit: r.pubkey(), raw: raw() });
        break;
      case 6:
        r.option(r.pubkey);
        actions.push({ type: 'SetArchivalAuthority', raw: raw() });
        break;
      default:
        throw new Error(`unknown SettingsAction tag ${tag}`);
    }
  }
  const memo = r.option(r.str);
  if (!r.done()) throw new Error('trailing bytes after SyncSettingsTransactionArgs');
  return { disc, numSigners, actions, memo };
}

/** Applies decoded settings actions the way the program does (signer list + threshold). */
export function applyActions(
  start: { signers: readonly { key: Address; mask: number }[]; threshold: number },
  actions: readonly SettingsAction[],
) {
  let signers = [...start.signers];
  let threshold = start.threshold;
  for (const a of actions) {
    if (a.type === 'AddSigner') {
      if (signers.some((s) => s.key === a.key)) throw new Error(`AddSigner of existing key ${a.key}`);
      signers.push({ key: a.key, mask: a.mask });
    } else if (a.type === 'RemoveSigner') {
      if (!signers.some((s) => s.key === a.key)) throw new Error(`RemoveSigner of absent key ${a.key}`);
      signers = signers.filter((s) => s.key !== a.key);
    } else if (a.type === 'ChangeThreshold') {
      threshold = a.threshold;
    }
  }
  return { signers, threshold };
}

export interface CompiledInner {
  programIdIndex: number;
  accountIndexes: number[];
  data: Uint8Array;
}
/** executeTransactionSync data: disc, accountIndex u8, numSigners u8, bytes(SmallVec<u8, CompiledInner>). */
export function decodeVaultSyncData(data: Uint8Array) {
  const r = reader(data);
  const disc = Buffer.from(r.take(8)).toString('hex');
  const accountIndex = r.u8();
  const numSigners = r.u8();
  const len = r.u32();
  const inner = reader(r.take(len));
  if (!r.done()) throw new Error('trailing bytes after SyncTransactionArgs');
  const instructions: CompiledInner[] = Array.from({ length: inner.u8() }, () => ({
    programIdIndex: inner.u8(),
    accountIndexes: Array.from(inner.take(inner.u8())),
    data: Uint8Array.from(inner.take(inner.u16())),
  }));
  if (!inner.done()) throw new Error('trailing bytes inside compiled instructions');
  return { disc, accountIndex, numSigners, instructions };
}

export function decodeCreateData(data: Uint8Array) {
  const r = reader(data);
  const disc = Buffer.from(r.take(8)).toString('hex');
  const settingsAuthority = r.option(r.pubkey);
  const threshold = r.u16();
  const signers = r.vec(() => ({ key: r.pubkey(), mask: r.u8() }));
  const timeLock = r.u32();
  const rentCollector = r.option(r.pubkey);
  const memo = r.option(r.str);
  if (!r.done()) throw new Error('trailing bytes after CreateSmartAccountArgs');
  return { disc, settingsAuthority, threshold, signers, timeLock, rentCollector, memo };
}

// ---------- SPL token account data ----------
const coption = (v: Uint8Array | null, len: number) => (v === null ? new Uint8Array(4 + len) : concat(u32(1), v));
export const mintData = (decimals: number, authority: Address | null = null, supply = 0n) =>
  concat(coption(authority ? pubkey(authority) : null, 32), u64(supply), u8(decimals), u8(1), coption(null, 32));
export const tokenAccountData = (mint: Address, owner: Address, amount: bigint) =>
  concat(pubkey(mint), pubkey(owner), u64(amount), coption(null, 32), u8(1), coption(null, 8), u64(0n), coption(null, 32));
export const tokenAmountOf = (data: Uint8Array) => reader(data.subarray(64, 72)).u64();
