// Implementation checks: our encoders reproduce Grid's Rust vectors byte for byte.
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  type Address,
  address,
  decompileTransactionMessage,
  getCompiledTransactionMessageDecoder,
  getTransactionDecoder,
  type Instruction,
  isSignerRole,
  isWritableRole,
} from '@solana/kit';

import {
  type AccountSnapshot,
  buildCreateAccount,
  buildRemoveSpendingLimit,
  buildSetSigners,
  buildVaultTransaction,
  deriveProgramConfig,
  deriveSettings,
  deriveSpendingLimit,
  deriveVault,
  type Permission,
  type SettingsAddress,
  wrapTransaction,
} from '../../src/index.ts';
import { decodeProgramConfig, decodeSettings, decodeSpendingLimit, permissionsOf } from '../../src/encoding/accounts.ts';
import { fromHex } from '../../src/encoding/bytes.ts';
import { hex } from '../support/util.ts';
import { setSignersActions } from '../../src/build/settings.ts';
import { encodeSettingsAction } from '../../src/encoding/instructions.ts';

const DIR = fileURLToPath(new URL('../vectors/grid/', import.meta.url));
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Json = any;
const load = (name: string): Json => JSON.parse(readFileSync(`${DIR}${name}`, 'utf8'));
const files = readdirSync(DIR).filter((f) => f.endsWith('.json'));
const byPrefix = (p: string) => files.filter((f) => f.startsWith(p));

type FlatMeta = { pubkey: string; isSigner: boolean; isWritable: boolean };
const flat = (ix: Instruction) => ({
  programId: ix.programAddress as string,
  accounts: (ix.accounts ?? []).map(
    (m): FlatMeta => ({ pubkey: m.address, isSigner: isSignerRole(m.role), isWritable: isWritableRole(m.role) }),
  ),
  dataHex: hex(new Uint8Array(ix.data ?? [])),
});
const expectedIx = (e: Json) => ({ programId: e.programId, accounts: e.accounts, dataHex: e.dataHex });

const ALL: Permission[] = ['initiate', 'vote', 'execute'];
const gridPerm = (p: string): Permission => ({ CAN_INITIATE: 'initiate', CAN_VOTE: 'vote', CAN_EXECUTE: 'execute' })[p] as Permission;

function snapshot(settings: string, signers: { key: string; permissions: readonly Permission[] }[], threshold = 1): AccountSnapshot {
  return {
    settings: address(settings) as SettingsAddress,
    slot: 0n,
    threshold,
    timeLock: 0,
    settingsAuthority: null,
    signers: signers.map((s) => ({ key: address(s.key), permissions: s.permissions })),
    transactionIndex: 0n,
  };
}

const kitIx = (j: Json): Instruction => ({
  programAddress: address(j.programId),
  accounts: j.accounts.map((a: FlatMeta) => ({
    address: address(a.pubkey),
    role: (a.isSigner ? 2 : 0) | (a.isWritable ? 1 : 0),
  })),
  data: fromHex(j.dataHex),
});

describe('vectors: executeTransactionSync', () => {
  for (const f of byPrefix('sync_')) {
    it(f, async () => {
      const v = load(f);
      const signers = v.input.transactionSigners.map((k: string) => ({ key: k, permissions: ALL }));
      const account = snapshot(v.input.settingsAddress, signers);
      assert.equal(await deriveVault(account.settings, v.input.vaultIndex), v.input.smartAccountAddress);
      const ix = await buildVaultTransaction({
        account,
        vaultIndex: v.input.vaultIndex,
        signers: v.input.transactionSigners.map((k: string) => address(k)),
        instructions: v.input.instructions.map(kitIx),
      });
      assert.deepEqual(flat(ix), expectedIx(v.expected.instructions[0]));
    });
  }
});

describe('vectors: wrapTransaction', () => {
  for (const f of byPrefix('wrap_')) {
    it(f, async () => {
      const v = load(f);
      const tx = getTransactionDecoder().decode(Buffer.from(v.input.transactionBase64, 'base64'));
      const message = decompileTransactionMessage(getCompiledTransactionMessageDecoder().decode(tx.messageBytes));
      const signers = v.input.transactionSigners.map((k: string) => ({ key: k, permissions: ALL }));
      const { instruction, stripped } = await wrapTransaction({
        account: snapshot(v.input.settingsAddress, signers),
        vaultIndex: 0,
        signers: v.input.transactionSigners.map((k: string) => address(k)),
        message,
      });
      assert.deepEqual(flat(instruction), expectedIx(v.expected.instructions[0]));
      const cb = v.expected.decompiledInstructions.filter((i: Json) => i.programId.startsWith('ComputeBudget'));
      const limitIx = cb.find((i: Json) => i.dataHex.startsWith('02'));
      if (limitIx) assert.equal(stripped.unitLimit, Buffer.from(limitIx.dataHex, 'hex').readUInt32LE(1));
      else assert.equal(stripped.unitLimit, undefined);
      if (v.expected.originalMicroLamportsPerCu !== undefined && v.expected.originalMicroLamportsPerCu !== null) {
        assert.equal(stripped.unitPrice, BigInt(v.expected.originalMicroLamportsPerCu));
      }
    });
  }
});

describe('vectors: executeSettingsTransactionSync', () => {
  const signerShaped = byPrefix('settings_').filter((f) => {
    const i = load(f).input;
    return (
      i.request?.signers &&
      i.request.timeLock === null &&
      i.request.adminAddress === '<missing>' &&
      i.settingsAuthority === '11111111111111111111111111111111'
    );
  });
  assert.ok(signerShaped.length >= 8, 'expected signer-change vectors');
  for (const f of signerShaped) {
    it(f, () => {
      const v = load(f);
      const i = v.input;
      const current = i.currentSigners.map((s: Json) => ({ key: s.address, permissions: permissionsOf(s.mask) }));
      const account = snapshot(i.settingsAddress, current, i.currentThreshold);
      const newSigners = i.request.signers.map((s: Json) => ({
        key: address(s.address),
        permissions: s.permissions.map(gridPerm),
      }));
      const newThreshold = i.request.threshold ?? i.currentThreshold;
      const ix = buildSetSigners({
        account,
        signers: i.transactionSigners.map((k: string) => address(k)),
        newSigners,
        newThreshold,
        rentPayer: address(i.rentPayer),
      });
      if (f === 'settings_permission_reorder_spurious_update.json') {
        // Grid compares permission lists, so a reorder becomes Remove+Add; we compare masks (spec §5: "permissions changed").
        const actions = setSignersActions(account.signers, newSigners, newThreshold).map(encodeSettingsAction).map(hex);
        assert.deepEqual(actions, ['020100', '020100']);
        return;
      }
      assert.deepEqual(flat(ix), expectedIx(v.expected.instructions[0]));
    });
  }

  it('settings_remove_spending_limit.json', () => {
    const v = load('settings_remove_spending_limit.json');
    const i = v.input;
    const account = snapshot(i.settingsAddress, i.transactionSigners.map((k: string) => ({ key: k, permissions: ALL })));
    const ix = buildRemoveSpendingLimit({
      account,
      signers: i.transactionSigners.map((k: string) => address(k)),
      spendingLimit: address(i.actions[0].spendingLimit),
      rentPayer: address(i.rentPayer),
    });
    assert.deepEqual(flat(ix), expectedIx(v.expected.instructions[0]));
  });
});

describe('vectors: createSmartAccount', () => {
  for (const f of byPrefix('create_smart_account_')) {
    it(f, async () => {
      const v = load(f);
      const a = v.input.createArgs;
      const res = await buildCreateAccount({
        programConfig: { smartAccountIndex: BigInt(v.input.programConfig.smartAccountIndex), treasury: address(v.input.programConfig.treasury) },
        creator: address(v.input.creator),
        signers: a.signers.map((s: Json) => ({ key: address(s.key), permissions: permissionsOf(s.mask) })),
        threshold: a.threshold,
        timeLock: a.timeLock,
        settingsAuthority: a.settingsAuthority === null ? null : address(a.settingsAuthority),
        rentCollector: a.rentCollector === null ? null : address(a.rentCollector),
        memo: a.memo,
        candidateCount: v.expected.candidateCount,
      });
      assert.equal(v.input.programConfig.address, await deriveProgramConfig());
      assert.deepEqual(flat(res.instruction), expectedIx(v.expected.instructions[0]));
      assert.deepEqual(res.candidates, v.expected.candidates.map((c: Json) => c.settingsAddress));
      assert.equal(res.computeUnitLimit, v.expected.computeUnitLimit);
    });
  }

  it('defaults to 25 candidates, accepts 59, rejects 0 and 60', async () => {
    const base = {
      programConfig: { smartAccountIndex: 41n, treasury: address('37Kf8hHRgsj5pm3u3rE8eRyLLk8WkaqGu6a4Q8kqPvyD') },
      creator: address('F6ptqu6PKt56WLT3MmqayimN4HL7b8PARFRwWQXi4Quk'),
      signers: [{ key: address('vwYrjUyZbzW4ysgdveAQTKngA5XPTr8e6aqs2T4QhSj'), permissions: ALL }],
      threshold: 1,
    };
    const v15 = load('create_smart_account_15_candidates.json');
    const r25 = await buildCreateAccount(base);
    assert.equal(r25.candidates.length, 25);
    assert.deepEqual(r25.candidates.slice(0, 15), v15.expected.candidates.map((c: Json) => c.settingsAddress));
    assert.equal((await buildCreateAccount({ ...base, candidateCount: 59 })).candidates.length, 59);
    for (const candidateCount of [0, 60, 1.5]) {
      await assert.rejects(buildCreateAccount({ ...base, candidateCount }), { code: 'INVALID_ARGUMENT' });
    }
  });
});

describe('vectors: PDAs', () => {
  for (const f of byPrefix('pda_')) {
    const v = load(f);
    const i = v.input;
    let derive: (() => Promise<Address>) | undefined;
    if (f.startsWith('pda_settings_')) derive = () => deriveSettings(BigInt(i.smartAccountIndex));
    else if (f.startsWith('pda_vault_')) derive = () => deriveVault(address(i.settingsAddress), i.accountIndex);
    else if (f === 'pda_spending_limit.json') derive = () => deriveSpendingLimit(address(i.settingsAddress), address(i.seed));
    else if (f === 'pda_program_config.json') derive = () => deriveProgramConfig();
    if (!derive) continue; // transaction/proposal/policy PDAs are out of scope
    const run = derive;
    it(f, async () => assert.equal(await run(), v.expected.address));
  }
  it('rejects vault index 256 and negative account index', async () => {
    await assert.rejects(deriveVault(address('41gqrPgijYycTaCCzKyLfvqikMEH9fzGCwZYAKQHvMbd'), 256), { code: 'INVALID_ARGUMENT' });
    await assert.rejects(deriveSettings(-1n), { code: 'INVALID_ARGUMENT' });
  });
});

describe('vectors: Borsh accounts', () => {
  for (const f of byPrefix('borsh_settings_')) {
    it(f, () => {
      const v = load(f);
      const i = v.input;
      const d = decodeSettings(fromHex(v.expected.accountDataHex));
      assert.equal(d.seed, BigInt(i.seed));
      assert.equal(d.settingsAuthority, i.settingsAuthority === '11111111111111111111111111111111' ? null : i.settingsAuthority);
      assert.equal(d.threshold, i.threshold);
      assert.equal(d.timeLock, i.timeLock);
      assert.equal(d.transactionIndex, BigInt(i.transactionIndex));
      assert.equal(d.staleTransactionIndex, BigInt(i.staleTransactionIndex));
      assert.equal(d.archivalAuthority, i.archivalAuthority);
      assert.equal(d.archivableAfter, BigInt(i.archivableAfter));
      assert.equal(d.bump, i.bump);
      assert.deepEqual(d.signers.map((s) => s.key), i.signers.map((s: Json) => s.key));
      assert.deepEqual(d.signerMasks, i.signers.map((s: Json) => s.mask));
      assert.equal(d.accountUtilization, i.accountUtilization);
      assert.equal(d.policySeed, i.policySeed === null ? null : BigInt(i.policySeed));
      assert.equal(d.reserved2, i.reserved2);
    });
  }
  it('borsh_spending_limit.json', () => {
    const v = load('borsh_spending_limit.json');
    const i = v.input;
    const d = decodeSpendingLimit(fromHex(v.expected.accountDataHex));
    assert.deepEqual(
      { ...d, amount: String(d.amount), remainingAmount: String(d.remainingAmount), lastReset: String(d.lastReset), expiration: String(d.expiration) },
      {
        settings: i.settings, seed: i.seed, accountIndex: i.accountIndex, mint: i.mint, amount: i.amount,
        period: 'month', remainingAmount: i.remainingAmount, lastReset: i.lastReset, bump: i.bump,
        signers: i.signers, destinations: i.destinations, expiration: i.expiration,
      },
    );
  });
  it('borsh_program_config.json', () => {
    const v = load('borsh_program_config.json');
    const d = decodeProgramConfig(fromHex(v.expected.accountDataHex));
    assert.deepEqual(d, {
      smartAccountIndex: BigInt(v.input.smartAccountIndex),
      authority: v.input.authority,
      smartAccountCreationFee: BigInt(v.input.smartAccountCreationFee),
      treasury: v.input.treasury,
    });
  });
});

describe('account decoders read in explicit wire order', () => {
  it('no reader call inside a returned object literal in src/encoding/accounts.ts', () => {
    const src = readFileSync(fileURLToPath(new URL('../../src/encoding/accounts.ts', import.meta.url)), 'utf8');
    const returns = [...src.matchAll(/return \{[^}]*\}/g)].map((m) => m[0]);
    assert.ok(returns.length >= 4, 'found the decoders\' return objects');
    for (const r of returns) assert.doesNotMatch(r, /\br\.\w+\(/, `fields are read before the object is built:\n${r}`);
    // Safeguard removed: the inline form is what the check rejects.
    assert.match('return { mint: r.pubkey(), owner: r.pubkey() }', /\br\.\w+\(/);
  });
});
