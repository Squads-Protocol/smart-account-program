// Seeded randomized round trips (spec §8 "Properties", 10k runs each). The seed is printed in the
// failure message; rerun with SDK_TEST_SEED=<seed>.
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  AccountRole,
  type Address,
  type Instruction,
  appendTransactionMessageInstructions,
  blockhash,
  createTransactionMessage,
  pipe,
  setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
} from '@solana/kit';

import { type CreateInput, type SetSignersInput, checkCreate, checkSetSigners, checkVaultTransaction } from './support/checks.ts';
import { type Rng, baseSeed, forAll } from './support/prng.ts';
import {
  COMPUTE_BUDGET_PROGRAM,
  PROGRAM_ID,
  SYSTEM_PROGRAM,
  concat,
  decodeSettingsSyncData,
  decodeVaultSyncData,
  option,
  pda,
  pubkey,
  settingsPdaWithBump,
  str,
  u128,
  u32,
  u64,
  u8,
  vaultPda,
  vec,
} from './support/program.ts';
import { type AccountSnapshot, type SignerEntry, pdaAddress, sdk } from './support/sdk.ts';
import { dataOf, entry, hex, noop, show } from './support/util.ts';

const RUNS = 10_000;
const TIMEOUT = 10 * 60_000;

// ---------- generators ----------
/** Masks for n signers that satisfy the signer-set rules (combined mask 7, at least one voter). */
function validMasks(r: Rng, n: number): number[] {
  const masks = Array.from({ length: n }, () => r.range(0, 7));
  const or = masks.reduce((a, b) => a | b, 0);
  if (or !== 7 || !masks.some((m) => m & 2)) masks[r.int(n)] = 7;
  return masks;
}
const voters = (s: readonly SignerEntry[]) => s.filter((x) => x.permissions.includes('vote')).length;
const snapshot = (settings: Address, signers: SignerEntry[], threshold: number): AccountSnapshot => ({
  settings,
  slot: 1n,
  threshold,
  timeLock: 0,
  settingsAuthority: null,
  signers,
  transactionIndex: 0n,
});

function genSetSigners(r: Rng): SetSignersInput {
  const pool = Array.from({ length: 8 }, () => r.address());
  const cur = r.shuffle(pool).slice(0, r.range(1, 5));
  const curMasks = validMasks(r, cur.length);
  const current = cur.map((k, i) => entry(k, curMasks[i]!));
  const next = r.shuffle(pool).slice(0, r.range(1, 5));
  const nextMasks = validMasks(r, next.length);
  // Bias towards keeping some keys unchanged and changing some permissions.
  const newSigners = next.map((k, i) => {
    const old = current.find((c) => c.key === k);
    return old !== undefined && r.bool(0.5) ? old : entry(k, nextMasks[i]!);
  });
  if (newSigners.reduce((m, s) => m | maskOf(s), 0) !== 7 || voters(newSigners) === 0) newSigners[0] = entry(newSigners[0]!.key, 7);
  const threshold = r.range(1, voters(current));
  return {
    account: snapshot(r.address(), current, threshold),
    signers: current.map((c) => noop(c.key)), // all members: combined mask 7, enough voters
    newSigners,
    newThreshold: r.range(1, voters(newSigners)),
    rentPayer: noop(r.address()),
  };
}
const maskOf = (s: SignerEntry) => (s.permissions.includes('initiate') ? 1 : 0) | (s.permissions.includes('vote') ? 2 : 0) | (s.permissions.includes('execute') ? 4 : 0);

/** A pool of settings/vault pairs, so the oracle derivation is paid once per pair. */
async function vaultPool(r: Rng, n: number) {
  return Promise.all(
    Array.from({ length: n }, async () => {
      const settings = r.address();
      const vaultIndex = r.bool(0.3) ? r.pick([0, 1, 255]) : r.range(0, 255);
      return { settings, vaultIndex, vault: await vaultPda(settings, vaultIndex) };
    }),
  );
}
const ROLES = [AccountRole.READONLY, AccountRole.WRITABLE];
function genInner(r: Rng, vault: Address, members: readonly Address[], count: number): Instruction[] {
  const programs = [SYSTEM_PROGRAM, ...Array.from({ length: 3 }, () => r.address())];
  const others = Array.from({ length: 6 }, () => r.address());
  return Array.from({ length: count }, () => ({
    programAddress: r.pick(programs),
    accounts: Array.from({ length: r.range(0, 6) }, () => {
      const k = r.int(10);
      if (k < 3) return { address: vault, role: r.pick([AccountRole.WRITABLE_SIGNER, AccountRole.READONLY_SIGNER]) };
      if (k < 4 && members.length > 0) return { address: r.pick(members), role: r.pick(ROLES) };
      return { address: r.pick(others), role: r.pick(ROLES) };
    }),
    data: r.bytes(r.range(0, 40)),
  }));
}
const cuLimitIx = (n: number): Instruction => ({ programAddress: COMPUTE_BUDGET_PROGRAM, data: concat(u8(2), u32(n)) });
const cuPriceIx = (n: bigint): Instruction => ({ programAddress: COMPUTE_BUDGET_PROGRAM, data: concat(u8(3), u64(n)) });

describe(`seeded randomized properties (SDK_TEST_SEED=${baseSeed}, ${RUNS} runs each)`, { timeout: TIMEOUT }, () => {
  it('buildSetSigners: actions decode and apply to exactly newSigners / newThreshold', async () => {
    await forAll(
      'setSigners round trip',
      RUNS,
      genSetSigners,
      async (c) => checkSetSigners(c, await sdk.buildSetSigners(c)),
      (c) => show({ current: c.account.signers, threshold: c.account.threshold, newSigners: c.newSigners, newThreshold: c.newThreshold }),
    );
  });

  it('buildVaultTransaction: inner instructions decode back (programs, accounts, data, order)', async (t) => {
    const r0 = (await import('./support/prng.ts')).rng(baseSeed);
    const pool = await vaultPool(r0, 32);
    await forAll(
      'vault tx round trip',
      RUNS,
      (r) => {
        const v = r.pick(pool);
        const members = Array.from({ length: r.range(1, 3) }, () => r.address());
        const signers = members.slice(0, r.range(1, members.length)).map(noop);
        return {
          ...v,
          account: snapshot(v.settings, members.map((k) => entry(k, 7)), signers.length),
          signers,
          instructions: genInner(r, v.vault, members, r.range(1, 6)),
        };
      },
      async (c) => checkVaultTransaction(c, await sdk.buildVaultTransaction(c)),
      (c) => show({ vaultIndex: c.vaultIndex, instructions: c.instructions.map((i) => [i.programAddress, i.accounts, i.data]) }),
    );
    t.diagnostic(`pool of ${pool.length} settings/vault pairs`);
  });

  it('wrapTransaction: keeps every non-ComputeBudget instruction in order and reports the stripped ones', async () => {
    const r0 = (await import('./support/prng.ts')).rng(baseSeed ^ 1);
    const pool = await vaultPool(r0, 32);
    await forAll(
      'wrap preserves meaning',
      RUNS,
      (r) => {
        const v = r.pick(pool);
        const member = r.address();
        const body = genInner(r, v.vault, [member], r.range(1, 5));
        const unitLimit = r.bool() ? r.range(0, 1_400_000) : undefined;
        const unitPrice = r.bool() ? r.bigBits(r.pick([8, 32, 64])) : undefined;
        const all = [...body];
        if (unitLimit !== undefined) all.splice(r.int(all.length + 1), 0, cuLimitIx(unitLimit));
        if (unitPrice !== undefined) all.splice(r.int(all.length + 1), 0, cuPriceIx(unitPrice));
        return { ...v, member, body, all, unitLimit, unitPrice, asMessage: r.bool() };
      },
      async (c) => {
        const base = { account: snapshot(c.settings, [entry(c.member, 7)], 1), vaultIndex: c.vaultIndex, signers: [noop(c.member)] };
        const res = c.asMessage
          ? await sdk.wrapTransaction({
              ...base,
              message: pipe(
                createTransactionMessage({ version: 0 }),
                (m) => setTransactionMessageFeePayer(c.member, m),
                (m) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: blockhash('CmpNeggWJ4JaWJeJ8YKN1Zypmk7uvQq3PECGUCAEMbky'), lastValidBlockHeight: 1n }, m),
                (m) => appendTransactionMessageInstructions(c.all, m),
              ),
            })
          : await sdk.wrapTransaction({ ...base, instructions: c.all });
        checkVaultTransaction({ ...base, vault: c.vault, instructions: c.body }, res.instruction);
        assert.equal(res.stripped.unitLimit === undefined ? undefined : BigInt(res.stripped.unitLimit), c.unitLimit === undefined ? undefined : BigInt(c.unitLimit), 'stripped.unitLimit');
        assert.equal(res.stripped.unitPrice === undefined ? undefined : BigInt(res.stripped.unitPrice), c.unitPrice, 'stripped.unitPrice');
        const fee = c.unitLimit !== undefined && c.unitPrice !== undefined ? (c.unitPrice * BigInt(c.unitLimit) + 999_999n) / 1_000_000n : undefined;
        assert.equal(res.stripped.priorityFeeLamports, fee, 'stripped.priorityFeeLamports = ceil(price x limit / 1e6)');
        // Same result as wrapping by hand: buildVaultTransaction on the non-ComputeBudget instructions.
        const direct = await sdk.buildVaultTransaction({ ...base, instructions: c.body });
        assert.equal(hex(dataOf(res.instruction)), hex(dataOf(direct)));
      },
      (c) => show({ asMessage: c.asMessage, unitLimit: c.unitLimit, unitPrice: c.unitPrice, n: c.all.length }),
    );
  });

  it('wrapTransaction: priorityFeeLamports is the total fee (1,000,000 micro-lamports/CU x 200,000 CU = 200,000 lamports)', async () => {
    const r0 = (await import('./support/prng.ts')).rng(baseSeed ^ 21);
    const [v] = await vaultPool(r0, 1);
    const member = r0.address();
    const body = genInner(r0, v!.vault, [member], 1);
    const base = { account: snapshot(v!.settings, [entry(member, 7)], 1), vaultIndex: v!.vaultIndex, signers: [noop(member)] };
    const res = await sdk.wrapTransaction({ ...base, instructions: [cuLimitIx(200_000), cuPriceIx(1_000_000n), ...body] });
    assert.equal(res.stripped.priorityFeeLamports, 200_000n);
    assert.equal(BigInt(res.stripped.unitLimit ?? -1), 200_000n, 'unitLimit kept');
    assert.equal(BigInt(res.stripped.unitPrice ?? -1), 1_000_000n, 'unitPrice kept');
    const odd = await sdk.wrapTransaction({ ...base, instructions: [cuLimitIx(3), cuPriceIx(1n), ...body] });
    assert.equal(odd.stripped.priorityFeeLamports, 1n, 'rounded up, never down to zero');
    const priceOnly = await sdk.wrapTransaction({ ...base, instructions: [cuPriceIx(5n), ...body] });
    assert.equal(priceOnly.stripped.priorityFeeLamports, undefined, 'needs both values');
    // Safeguard removed: passing unitPrice as the total would under-pay by a factor of unitLimit / 1e6.
    assert.notEqual(res.stripped.unitPrice, res.stripped.priorityFeeLamports, 'unitPrice is not the total fee');
  });

  it('buildCreateAccount: data decodes to the inputs; candidates and CU limit match the program seeds', async () => {
    const indexPool = [0n, 41n, 745_853n, (1n << 64n) - 2n, (1n << 64n), (1n << 128n) - 79n];
    const oracleCache = new Map<bigint, readonly [Address, number]>();
    const oracle = async (i: bigint) => {
      let v = oracleCache.get(i);
      if (v === undefined) oracleCache.set(i, (v = await settingsPdaWithBump(i)));
      return v;
    };
    await forAll(
      'create round trip',
      RUNS,
      (r) => {
        const keys = Array.from({ length: r.range(1, 5) }, () => r.address());
        const masks = validMasks(r, keys.length);
        const signers = keys.map((k, i) => entry(k, masks[i]!));
        const input: CreateInput & { candidateCount: number } = {
          programConfig: { address: r.address(), treasury: r.address(), smartAccountIndex: r.bool(0.8) ? r.pick(indexPool) + BigInt(r.int(20)) : r.bigBits(127) },
          creator: noop(r.address()),
          signers,
          threshold: r.range(1, voters(signers)),
          timeLock: r.bool() ? 0 : r.u32(),
          settingsAuthority: r.bool() ? null : r.address(),
          rentCollector: r.bool() ? null : r.address(),
          memo: r.bool() ? null : Array.from({ length: r.range(0, 24) }, () => r.pick(['a', 'Z', '7', ' ', 'é', '✓'])).join(''),
          candidateCount: r.bool(0.005) ? r.range(4, 59) : r.range(1, 3),
        };
        return input;
      },
      async (c) => {
        const res = await sdk.buildCreateAccount({ ...c, programConfig: { ...c.programConfig, authority: c.programConfig.treasury, smartAccountCreationFee: 0n } });
        const want = await Promise.all(Array.from({ length: c.candidateCount }, (_, k) => oracle(c.programConfig.smartAccountIndex + 1n + BigInt(k))));
        checkCreate(c, res, want);
      },
      (c) => show({ ...c, creator: c.creator.address }),
    );
  });

  it('deriveSettings / deriveVault equal kit getProgramDerivedAddress with the program seeds', async () => {
    await forAll(
      'derivations',
      RUNS,
      (r) => ({ settings: r.address(), vaultIndex: r.range(0, 255), index: r.bool(0.1) ? r.pick([0n, (1n << 128n) - 1n]) : r.bigBits(r.pick([8, 64, 128])), vault: r.bool() }),
      async (c) => {
        if (c.vault) {
          const [want] = await pda(PROGRAM_ID, ['smart_account', pubkey(c.settings), 'smart_account', u8(c.vaultIndex)]);
          assert.equal(pdaAddress(await sdk.deriveVault(c.settings, c.vaultIndex)), want);
        } else {
          const [want] = await pda(PROGRAM_ID, ['smart_account', 'settings', u128(c.index)]);
          assert.equal(pdaAddress(await sdk.deriveSettings(c.index)), want);
        }
      },
      (c) => show(c),
    );
  });
});

// ---------- the checks have teeth (safeguard-removed counterparts) ----------
describe('round-trip checks reject corrupted output', () => {
  it('buildSetSigners: dropping, reordering or editing an action is caught', async () => {
    const { rng } = await import('./support/prng.ts');
    const r = rng(baseSeed ^ 7);
    let c = genSetSigners(r);
    while (!c.account.signers.some((s) => !c.newSigners.some((n) => n.key === s.key))) c = genSetSigners(r);
    const ix = await sdk.buildSetSigners(c);
    checkSetSigners(c, ix);
    const d = decodeSettingsSyncData(dataOf(ix));
    const reencode = (acts: typeof d.actions) => ({
      ...ix,
      data: concat(dataOf(ix).subarray(0, 9), vec(acts.map((a) => a.raw)), option(d.memo === null ? null : str(d.memo))),
    });
    const removeIdx = d.actions.findIndex((a) => a.type === 'RemoveSigner');
    const mutants = {
      'drop a RemoveSigner': d.actions.filter((_, i) => i !== removeIdx),
      'move a RemoveSigner first': [d.actions[removeIdx]!, ...d.actions.filter((_, i) => i !== removeIdx)],
      'drop the final ChangeThreshold': d.actions.slice(0, -1),
    };
    for (const [name, acts] of Object.entries(mutants)) assert.throws(() => checkSetSigners(c, reencode(acts)), Error, name);
  });

  it('buildVaultTransaction: swapping inner data or keeping a ComputeBudget instruction is caught', async () => {
    const settings = (await import('./support/prng.ts')).rng(baseSeed ^ 9).address();
    const vault = await vaultPda(settings, 0);
    const member = (await import('./support/prng.ts')).rng(baseSeed ^ 11).address();
    const body: Instruction[] = [
      { programAddress: SYSTEM_PROGRAM, accounts: [{ address: vault, role: AccountRole.WRITABLE_SIGNER }], data: Uint8Array.of(1) },
      { programAddress: SYSTEM_PROGRAM, accounts: [{ address: vault, role: AccountRole.WRITABLE_SIGNER }], data: Uint8Array.of(2) },
    ];
    const input = { account: snapshot(settings, [entry(member, 7)], 1), vaultIndex: 0, vault, signers: [noop(member)], instructions: body };
    const ix = await sdk.buildVaultTransaction(input);
    checkVaultTransaction(input, ix);
    const swapped = await sdk.buildVaultTransaction({ ...input, instructions: [body[1]!, body[0]!] });
    assert.throws(() => checkVaultTransaction(input, swapped), /inner 0 data/);
    const kept = await sdk.buildVaultTransaction({ ...input, instructions: [cuLimitIx(5), ...body] });
    assert.throws(() => checkVaultTransaction(input, kept));
    assert.equal(decodeVaultSyncData(dataOf(kept)).instructions.length, 3);
  });
});
