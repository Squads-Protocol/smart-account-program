// Implementation checks: builder preconditions and settings-action ordering.
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { AccountRole, type Address, address, generateKeyPairSigner, type Instruction } from '@solana/kit';

import {
  type AccountSnapshot,
  buildCreateAccount,
  buildRemoveSpendingLimit,
  buildSetSigners,
  buildTransfer,
  buildVaultTransaction,
  deriveVault,
  type Permission,
  type SettingsAddress,
} from '../../src/index.ts';
import { setSignersActions } from '../../src/build/settings.ts';

const ALL: Permission[] = ['initiate', 'vote', 'execute'];
const keys = await Promise.all(Array.from({ length: 6 }, async () => (await generateKeyPairSigner()).address));
const [A, B, C, D, E, X] = keys as [Address, Address, Address, Address, Address, Address];
const SETTINGS = address('41gqrPgijYycTaCCzKyLfvqikMEH9fzGCwZYAKQHvMbd') as SettingsAddress;
const SYSTEM = address('11111111111111111111111111111111');

const account = (over: Partial<AccountSnapshot> = {}): AccountSnapshot => ({
  settings: SETTINGS,
  slot: 1n,
  threshold: 2,
  timeLock: 0,
  settingsAuthority: null,
  signers: [
    { key: A, permissions: ALL },
    { key: B, permissions: ['vote'] },
    { key: C, permissions: ['initiate', 'execute'] },
  ],
  transactionIndex: 0n,
  ...over,
});
const vault = await deriveVault(SETTINGS, 0);
const transfer: Instruction = {
  programAddress: SYSTEM,
  accounts: [
    { address: vault, role: AccountRole.WRITABLE_SIGNER },
    { address: X, role: AccountRole.WRITABLE },
  ],
  data: new Uint8Array(12),
};
const vt = (over: Partial<AccountSnapshot>, signers: Address[], instructions = [transfer]) =>
  buildVaultTransaction({ account: account(over), vaultIndex: 0, signers, instructions });

describe('buildVaultTransaction preconditions', () => {
  it('accepts a valid signer set and keeps signer order', async () => {
    const ix = await vt({}, [B, A]);
    assert.deepEqual(ix.accounts?.slice(2, 4).map((m) => m.address), [B, A]);
    assert.equal(ix.data?.[9], 2, 'numSigners');
  });
  it('SYNC_NOT_ALLOWED with a time lock', async () => {
    await assert.rejects(vt({ timeLock: 60 }, [A, B]), { code: 'SYNC_NOT_ALLOWED' });
  });
  it('INSUFFICIENT_SIGNERS: below threshold, non-member, missing permission, duplicate, empty', async () => {
    await assert.rejects(vt({}, [A]), { code: 'INSUFFICIENT_SIGNERS' });
    await assert.rejects(vt({}, [A, X]), { code: 'INSUFFICIENT_SIGNERS' });
    await assert.rejects(vt({ threshold: 1 }, [B]), { code: 'INSUFFICIENT_SIGNERS' });
    await assert.rejects(vt({ threshold: 1 }, [C]), { code: 'INSUFFICIENT_SIGNERS' });
    await assert.rejects(vt({}, [A, A]), { code: 'INSUFFICIENT_SIGNERS' });
    await assert.rejects(vt({}, []), { code: 'INSUFFICIENT_SIGNERS' });
  });
  it('UNSUPPORTED_TRANSACTION for an inner signer that is neither the vault nor a signer, or a lookup-table account', async () => {
    const foreign: Instruction = { ...transfer, accounts: [{ address: X, role: AccountRole.WRITABLE_SIGNER }] };
    await assert.rejects(vt({}, [A, B], [foreign]), { code: 'UNSUPPORTED_TRANSACTION' });
    const lookup = { ...transfer, accounts: [{ address: X, role: AccountRole.READONLY, addressIndex: 0, lookupTableAddress: D }] };
    await assert.rejects(vt({}, [A, B], [lookup]), { code: 'UNSUPPORTED_TRANSACTION' });
  });
  it('INVALID_ARGUMENT for no instructions, bad vault index, non-positive amount, SPL without decimals', async () => {
    await assert.rejects(vt({}, [A, B], []), { code: 'INVALID_ARGUMENT' });
    await assert.rejects(buildVaultTransaction({ account: account(), vaultIndex: 256, signers: [A, B], instructions: [transfer] }), {
      code: 'INVALID_ARGUMENT',
    });
    const base = { account: account(), vaultIndex: 0, signers: [A, B], to: X };
    await assert.rejects(buildTransfer({ ...base, mint: 'SOL', amount: 0n }), { code: 'INVALID_ARGUMENT' });
    await assert.rejects(buildTransfer({ ...base, mint: D, amount: 1n }), { code: 'INVALID_ARGUMENT' });
  });
});

describe('buildSetSigners', () => {
  const current = [
    { key: A, permissions: ALL },
    { key: B, permissions: ALL },
    { key: C, permissions: ALL },
  ];
  it('orders ChangeThreshold(1), adds, updates as remove+add, removals in on-chain order, ChangeThreshold(n)', () => {
    const actions = setSignersActions(current, [
      { key: E, permissions: ALL },
      { key: B, permissions: ['vote'] },
      { key: D, permissions: ['vote'] },
    ], 2);
    assert.deepEqual(actions, [
      { type: 'changeThreshold', threshold: 1 },
      { type: 'addSigner', key: E, mask: 7 },
      { type: 'addSigner', key: D, mask: 2 },
      { type: 'removeSigner', key: B },
      { type: 'addSigner', key: B, mask: 2 },
      { type: 'removeSigner', key: A },
      { type: 'removeSigner', key: C },
      { type: 'changeThreshold', threshold: 2 },
    ]);
  });
  const acct = account({ threshold: 1, signers: current });
  const set = (newSigners: { key: Address; permissions: Permission[] }[], newThreshold: number, over: Partial<AccountSnapshot> = {}) =>
    () => buildSetSigners({ account: { ...acct, ...over }, signers: [A], newSigners, newThreshold, rentPayer: X });
  it('INVALID_SIGNER_SET: empty, duplicate, missing permission, threshold out of range, unknown permission', () => {
    assert.throws(set([], 1), { code: 'INVALID_SIGNER_SET' });
    assert.throws(set([{ key: D, permissions: ALL }, { key: D, permissions: ALL }], 1), { code: 'INVALID_SIGNER_SET' });
    assert.throws(set([{ key: D, permissions: ['vote', 'execute'] }], 1), { code: 'INVALID_SIGNER_SET' });
    assert.throws(set([{ key: D, permissions: ALL }], 2), { code: 'INVALID_SIGNER_SET' });
    assert.throws(set([{ key: D, permissions: ALL }], 0), { code: 'INVALID_SIGNER_SET' });
    assert.throws(set([{ key: D, permissions: ['admin' as Permission] }], 1), { code: 'INVALID_SIGNER_SET' });
  });
  it('SYNC_NOT_ALLOWED for a controlled account or a time lock; INSUFFICIENT_SIGNERS for a non-member', () => {
    assert.throws(set([{ key: D, permissions: ALL }], 1, { settingsAuthority: D }), { code: 'SYNC_NOT_ALLOWED' });
    assert.throws(set([{ key: D, permissions: ALL }], 1, { timeLock: 1 }), { code: 'SYNC_NOT_ALLOWED' });
    assert.throws(
      () => buildRemoveSpendingLimit({ account: acct, signers: [X], spendingLimit: D, rentPayer: X }),
      { code: 'INSUFFICIENT_SIGNERS' },
    );
  });
});

describe('buildCreateAccount preconditions', () => {
  it('INVALID_SIGNER_SET for a bad signer set; INVALID_ARGUMENT for a non-ProgramConfig account', async () => {
    const programConfig = { smartAccountIndex: 1n, treasury: X };
    await assert.rejects(buildCreateAccount({ programConfig, creator: X, signers: [{ key: A, permissions: ['vote'] }], threshold: 1 }), {
      code: 'INVALID_SIGNER_SET',
    });
    await assert.rejects(
      buildCreateAccount({ programConfig: { data: new Uint8Array(160) }, creator: X, signers: [{ key: A, permissions: ALL }], threshold: 1 }),
      { code: 'INVALID_ARGUMENT' },
    );
  });
});

describe('no dead code in src/encoding', () => {
  it('every export of an internal encoding module is used elsewhere in src', async () => {
    const { readFileSync, readdirSync } = await import('node:fs');
    const { fileURLToPath } = await import('node:url');
    const dir = (p: string) => fileURLToPath(new URL(`../../src/${p}`, import.meta.url));
    const files = [...readdirSync(dir('')).filter((f) => f.endsWith('.ts')), ...readdirSync(dir('build')).map((f) => `build/${f}`), ...readdirSync(dir('encoding')).map((f) => `encoding/${f}`)];
    const text = new Map(files.map((f) => [f, readFileSync(dir(f), 'utf8')]));
    const unused = (exported: string, name: string) =>
      ![...text].some(([f, t]) => (f === exported ? (t.match(new RegExp(`\\b${name}\\b`, 'g')) ?? []).length > 1 : new RegExp(`\\b${name}\\b`).test(t)));
    const dead = files
      .filter((f) => f.startsWith('encoding/'))
      .flatMap((f) => [...text.get(f)!.matchAll(/export (?:const|function|interface|type) (\w+)/g)].map((m) => `${f}:${m[1]}`))
      .filter((x) => unused(x.split(':')[0]!, x.split(':')[1]!));
    assert.deepEqual(dead, []);
    // Safeguard removed: an export nobody references is reported.
    text.set('encoding/probe.ts', 'export const neverUsedProbe = 1;');
    assert.equal(unused('encoding/probe.ts', 'neverUsedProbe'), true);
  });
});
