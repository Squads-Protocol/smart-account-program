// Bounded exhaustive tests of the signer-set preconditions (spec §5, §8 "Pure logic"): every signer
// set up to 5 keys x every permission mask x every threshold 0..n+1. Each case's expected outcome
// (success, or the §4 code(s) of the violated rule) comes from an oracle written from the spec; a
// seeded sample of successful cases is then executed in LiteSVM to check the post-state.
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { AccountRole, type Address, type Instruction, type KeyPairSigner, generateKeyPairSigner, lamports } from '@solana/kit';

import { checkCreate, checkSetSigners, checkVaultTransaction } from './support/checks.ts';
import { baseSeed, rng } from './support/prng.ts';
import { SYSTEM_PROGRAM, concat, decodeSettingsAccount, settingsPda, settingsPdaWithBump, u32, u64, vaultPda } from './support/program.ts';
import { type AccountSnapshot, ERROR_CODES, type ErrorCode, type SignerEntry, sdk } from './support/sdk.ts';
import { createSvmRpc, execute, readProgramConfig, setupSvm } from './support/svm.ts';
import { assertSdkError, entry, noop, settle, show, signerSetKey } from './support/util.ts';

const TIMEOUT = 10 * 60_000;
const VOTE = 2;
const keyRng = rng((baseSeed ^ 0x51) >>> 0);
const KEYS = Array.from({ length: 6 }, () => keyRng.address());
const POOL = Array.from({ length: 6 }, () => keyRng.address());
const or = (m: readonly number[]) => m.reduce((a, b) => a | b, 0);
const voteCount = (m: readonly number[]) => m.filter((x) => x & VOTE).length;

// ---------- oracles (spec §5) ----------
/** newSigners/threshold rules of buildSetSigners and buildCreateAccount. */
function signerSetRules(masks: readonly number[], duplicate: boolean, threshold: number): ErrorCode[] {
  const ok = masks.length > 0 && !duplicate && or(masks) === 7 && threshold >= 1 && threshold <= voteCount(masks);
  return ok ? [] : ['INVALID_SIGNER_SET'];
}
/** buildVaultTransaction: timeLock 0; at least `threshold` voters among the signers; combined mask 7. */
function vaultRules(masks: readonly number[], threshold: number, timeLock: number): ErrorCode[] {
  const codes: ErrorCode[] = [];
  if (timeLock > 0) codes.push('SYNC_NOT_ALLOWED');
  if (voteCount(masks) < threshold || or(masks) !== 7) codes.push('INSUFFICIENT_SIGNERS');
  return codes;
}

// ---------- enumeration ----------
/** All mask tuples of length n when n ≤ orderedUpTo, else all multisets (sorted tuples): the rules are order-free. */
function* tuples(n: number, orderedUpTo: number): Generator<number[]> {
  if (n <= orderedUpTo) {
    for (let i = 0; i < 8 ** n; i++) yield Array.from({ length: n }, (_, k) => Math.floor(i / 8 ** k) % 8);
    return;
  }
  const rec = function* (prefix: number[], min: number): Generator<number[]> {
    if (prefix.length === n) yield prefix;
    else for (let m = min; m < 8; m++) yield* rec([...prefix, m], m);
  };
  yield* rec([], 0);
}
interface SetCase {
  masks: number[];
  duplicate: boolean;
  threshold: number;
}
function* signerSetCases(orderedUpTo: number): Generator<SetCase> {
  for (let n = 0; n <= 5; n++) {
    for (const masks of tuples(n, orderedUpTo)) {
      for (let threshold = 0; threshold <= n + 1; threshold++) {
        yield { masks, duplicate: false, threshold };
        if (n >= 2 && threshold === Math.max(1, Math.min(n, voteCount(masks)))) yield { masks, duplicate: true, threshold };
      }
    }
  }
}
/** Key 0 is the account's current member (so its mask either stays 7 or changes); the rest are new. */
const keyFor = (i: number) => (i === 0 ? KEYS[0]! : POOL[i]!);
const toEntries = (c: SetCase): SignerEntry[] =>
  c.masks.map((m, i) => entry(keyFor(c.duplicate && i === c.masks.length - 1 ? 0 : i), m));

const snapshot = (p: Partial<AccountSnapshot>): AccountSnapshot => ({
  settings: KEYS[5]!,
  slot: 1n,
  threshold: 1,
  timeLock: 0,
  settingsAuthority: null,
  signers: [entry(KEYS[0]!, 7)],
  transactionIndex: 0n,
  ...p,
});

/** Runs every case; collects mismatches instead of stopping at the first. */
async function grid<C>(cases: Iterable<C>, expected: (c: C) => ErrorCode[], run: (c: C) => unknown, checkOk: (c: C, v: never) => void) {
  const bad: string[] = [];
  let total = 0;
  let oks = 0;
  for (const c of cases) {
    total++;
    const want = expected(c);
    const r = await settle(() => run(c));
    try {
      if (want.length === 0) {
        if (!r.ok) throw new Error(`expected success, got ${String(r.error)}`);
        oks++;
        checkOk(c, r.value as never);
      } else {
        if (r.ok) throw new Error(`expected ${want.join('|')}, but it succeeded`);
        assertSdkError(r.error, want);
      }
    } catch (e) {
      if (bad.length < 10) bad.push(`${show(c)}: ${e instanceof Error ? e.message : String(e)}`);
      else bad[10] = '...';
    }
  }
  assert.deepEqual(bad, [], `mismatches (of ${total} cases)`);
  return { total, oks };
}

describe('signer-set preconditions, bounded exhaustive', { timeout: TIMEOUT }, () => {
  it('buildSetSigners: all signer sets ≤ 5 keys x 8 masks x thresholds 0..n+1', async (t) => {
    const account = snapshot({});
    const signers = [noop(KEYS[0]!)];
    const rentPayer = noop(KEYS[1]!);
    const { total, oks } = await grid(
      signerSetCases(5),
      (c) => signerSetRules(c.masks, c.duplicate, c.threshold),
      (c) => sdk.buildSetSigners({ account, signers, newSigners: toEntries(c), newThreshold: c.threshold, rentPayer }),
      (c, ix: Instruction) => checkSetSigners({ account, signers, newSigners: toEntries(c), newThreshold: c.threshold, rentPayer }, ix),
    );
    t.diagnostic(`${total} cases, ${oks} valid`);
    assert.ok(total > 250_000);
  });

  it('buildCreateAccount: same rules (ordered ≤ 3 keys, multisets for 4-5 keys)', async (t) => {
    const programConfig = { address: KEYS[2]!, smartAccountIndex: 41n, treasury: KEYS[3]!, authority: KEYS[3]!, smartAccountCreationFee: 0n };
    const oracle = [await settingsPdaWithBump(42n)] as const;
    const base = { programConfig, creator: noop(KEYS[4]!), timeLock: 0, settingsAuthority: null, rentCollector: null, memo: null, candidateCount: 1 };
    const { total, oks } = await grid(
      signerSetCases(3),
      (c) => signerSetRules(c.masks, c.duplicate, c.threshold),
      (c) => sdk.buildCreateAccount({ ...base, signers: toEntries(c), threshold: c.threshold }),
      (c, res: Parameters<typeof checkCreate>[1]) => checkCreate({ ...base, signers: toEntries(c), threshold: c.threshold }, res, oracle),
    );
    t.diagnostic(`${total} cases, ${oks} valid`);
  });

  it('buildVaultTransaction: signer subsets ≤ 5 keys x 8 masks x account thresholds 1..n+1 x time lock {0, 60}', async (t) => {
    const extra = entry(KEYS[0]!, 7); // a member that does not sign
    const vault = await vaultPda(KEYS[5]!, 0);
    const inner: Instruction[] = [{ programAddress: SYSTEM_PROGRAM, accounts: [{ address: vault, role: AccountRole.WRITABLE_SIGNER }, { address: KEYS[1]!, role: AccountRole.WRITABLE }], data: concat(u32(2), u64(1n)) }];
    const cases = function* () {
      for (let n = 0; n <= 5; n++)
        for (const masks of tuples(n, 3))
          for (let threshold = 1; threshold <= n + 1; threshold++) for (const timeLock of [0, 60]) yield { masks, threshold, timeLock };
    };
    const argsOf = (c: { masks: number[]; threshold: number; timeLock: number }) => {
      const members = c.masks.map((m, i) => entry(POOL[i]!, m));
      return {
        account: snapshot({ signers: [...members, extra], threshold: c.threshold, timeLock: c.timeLock }),
        vaultIndex: 0,
        signers: members.map((m) => noop(m.key)),
        instructions: inner,
      };
    };
    const { total, oks } = await grid(
      cases(),
      (c) => vaultRules(c.masks, c.threshold, c.timeLock),
      (c) => sdk.buildVaultTransaction(argsOf(c)),
      (c, ix: Instruction) => checkVaultTransaction({ ...argsOf(c), vault }, ix),
    );
    t.diagnostic(`${total} cases, ${oks} valid`);
  });

  it('buildVaultTransaction: a signer that is not a member is refused', async () => {
    const r = await settle(() =>
      sdk.buildVaultTransaction({ account: snapshot({}), vaultIndex: 0, signers: [noop(KEYS[0]!), noop(POOL[3]!)], instructions: [] }),
    );
    assert.equal(r.ok, false);
    if (!r.ok) assertSdkError(r.error, ['INSUFFICIENT_SIGNERS', 'INVALID_SIGNER_SET']);
  });

  it('buildSetSigners: current signers must reach the account threshold (3 members x all masks x thresholds x subsets)', async (t) => {
    const cases = function* () {
      for (const masks of tuples(3, 3)) for (let threshold = 1; threshold <= 3; threshold++) for (let sub = 0; sub < 8; sub++) yield { masks, threshold, sub };
    };
    const argsOf = (c: { masks: number[]; threshold: number; sub: number }) => {
      const members = c.masks.map((m, i) => entry(KEYS[i]!, m));
      return {
        account: snapshot({ signers: members, threshold: c.threshold }),
        signers: members.filter((_, i) => (c.sub >> i) & 1).map((m) => noop(m.key)),
        newSigners: [entry(POOL[0]!, 7)],
        newThreshold: 1,
        rentPayer: noop(POOL[1]!),
      };
    };
    const signing = (c: { masks: number[]; sub: number }) => c.masks.filter((_, i) => (c.sub >> i) & 1);
    const { total } = await grid(
      cases(),
      (c) => (voteCount(signing(c)) < c.threshold || or(signing(c)) !== 7 ? ['INSUFFICIENT_SIGNERS'] : []),
      (c) => sdk.buildSetSigners(argsOf(c)),
      (c, ix: Instruction) => checkSetSigners(argsOf(c), ix),
    );
    t.diagnostic(`${total} cases`);
  });

  it('buildSetSigners refuses a controlled account (autonomous only) and a time-locked one', async () => {
    const base = { signers: [noop(KEYS[0]!)], newSigners: [entry(POOL[0]!, 7)], newThreshold: 1, rentPayer: noop(POOL[1]!) };
    const controlled = await settle(() => sdk.buildSetSigners({ ...base, account: snapshot({ settingsAuthority: KEYS[4]! }) }));
    assert.equal(controlled.ok, false, 'controlled account refused');
    if (!controlled.ok) assertSdkError(controlled.error, ERROR_CODES);
    const locked = await settle(() => sdk.buildSetSigners({ ...base, account: snapshot({ timeLock: 60 }) }));
    assert.equal(locked.ok, false, 'time-locked account refused (sync needs time lock 0)');
    if (!locked.ok) assertSdkError(locked.error, 'SYNC_NOT_ALLOWED');
  });
});

describe('the case grid discriminates every rule (safeguard-removed oracles disagree somewhere)', () => {
  const mutants: Record<string, (c: SetCase) => boolean> = {
    'no non-empty check': (c) => !c.duplicate && (c.masks.length === 0 || (or(c.masks) === 7 && c.threshold >= 1 && c.threshold <= voteCount(c.masks))),
    'no duplicate check': (c) => c.masks.length > 0 && or(c.masks) === 7 && c.threshold >= 1 && c.threshold <= voteCount(c.masks),
    'no mask check': (c) => c.masks.length > 0 && !c.duplicate && c.threshold >= 1 && c.threshold <= voteCount(c.masks),
    'no lower threshold bound': (c) => c.masks.length > 0 && !c.duplicate && or(c.masks) === 7 && c.threshold <= voteCount(c.masks),
    'threshold bound by keys, not voters': (c) => c.masks.length > 0 && !c.duplicate && or(c.masks) === 7 && c.threshold >= 1 && c.threshold <= c.masks.length,
  };
  for (const [name, mutant] of Object.entries(mutants)) {
    it(name, () => {
      let differ = 0;
      for (const c of signerSetCases(3)) if (mutant(c) !== (signerSetRules(c.masks, c.duplicate, c.threshold).length === 0)) differ++;
      assert.ok(differ > 0, `${name}: the grid has no case that tells it apart`);
    });
  }
});

describe('sampled cases executed on the mainnet binary: post-state equals the request', { timeout: TIMEOUT }, () => {
  const sample = (n: number, seedTag: number) => {
    const r = rng((baseSeed ^ seedTag) >>> 0);
    const valid = [...signerSetCases(3)].filter((c) => !c.duplicate && signerSetRules(c.masks, false, c.threshold).length === 0 && c.masks.length > 0);
    return Array.from({ length: n }, () => r.pick(valid));
  };

  it(`buildSetSigners from the email shape (seed ${baseSeed})`, async () => {
    const r = rng((baseSeed ^ 0xabc) >>> 0);
    for (const c of sample(8, 0x77)) {
      const { svm, programConfig } = await setupSvm();
      const payer = await generateKeyPairSigner();
      svm.airdrop(payer.address, lamports(10_000_000_000n));
      const [privy, turnkey, ...fresh] = await Promise.all(Array.from({ length: 6 }, () => generateKeyPairSigner()));
      const settings = await settingsPda(programConfig.smartAccountIndex + 1n);
      const { createSmartAccountIx } = await import('./support/program.ts');
      await execute(svm, payer, [
        createSmartAccountIx({ programConfig: programConfig.address, treasury: programConfig.treasury, creator: payer, signers: [{ key: privy!.address, mask: 7 }, { key: turnkey!.address, mask: 7 }], threshold: 1, candidates: [settings] }),
      ]);
      // New keys: a mix of the current keys (permission changes) and fresh ones.
      const candidates: KeyPairSigner[] = r.shuffle([privy!, turnkey!, ...fresh]);
      const newSigners = c.masks.map((m, i) => entry(candidates[i]!.address, m));
      const account = await sdk.fetchAccount(createSvmRpc(svm).rpc, settings);
      const ix = await sdk.buildSetSigners({ account, signers: [privy!], newSigners, newThreshold: c.threshold, rentPayer: payer });
      await execute(svm, payer, [ix], [privy!]);
      const acc = svm.getAccount(settings);
      assert.ok(acc.exists);
      const after = decodeSettingsAccount(acc.data);
      assert.deepEqual(signerSetKey(after.signers), signerSetKey(c.masks.map((m, i) => ({ key: candidates[i]!.address, mask: m }))), show(c));
      assert.equal(after.threshold, c.threshold);
    }
  });

  it('buildCreateAccount creates exactly the requested signer set', async () => {
    for (const c of sample(4, 0x99)) {
      const { svm, programConfig } = await setupSvm();
      const payer = await generateKeyPairSigner();
      svm.airdrop(payer.address, lamports(10_000_000_000n));
      const keys = await Promise.all(c.masks.map(() => generateKeyPairSigner()));
      const res = await sdk.buildCreateAccount({
        programConfig, creator: payer, signers: c.masks.map((m, i) => entry(keys[i]!.address, m)), threshold: c.threshold,
        timeLock: 0, settingsAuthority: null, rentCollector: null, memo: null, candidateCount: 2,
      });
      await execute(svm, payer, [res.instruction]);
      const acc = svm.getAccount(res.candidates[0]!);
      assert.ok(acc.exists);
      const s = decodeSettingsAccount(acc.data);
      assert.deepEqual(signerSetKey(s.signers), signerSetKey(c.masks.map((m, i) => ({ key: keys[i]!.address, mask: m }))));
      assert.equal(s.threshold, c.threshold);
      assert.equal(readProgramConfig(svm, programConfig.address).smartAccountIndex, programConfig.smartAccountIndex + 1n);
    }
  });

  it('buildVaultTransaction executes with valid signer sets', async () => {
    for (const c of sample(4, 0x55)) {
      const { svm, programConfig } = await setupSvm();
      const payer = await generateKeyPairSigner();
      svm.airdrop(payer.address, lamports(10_000_000_000n));
      const keys = await Promise.all(c.masks.map(() => generateKeyPairSigner()));
      const settings = await settingsPda(programConfig.smartAccountIndex + 1n);
      const { createSmartAccountIx } = await import('./support/program.ts');
      const members = c.masks.map((m, i) => ({ key: keys[i]!.address, mask: m }));
      await execute(svm, payer, [createSmartAccountIx({ programConfig: programConfig.address, treasury: programConfig.treasury, creator: payer, signers: members, threshold: c.threshold, candidates: [settings] })]);
      const vault = await vaultPda(settings, 0);
      svm.airdrop(vault, lamports(1_000_000_000n));
      const to: Address = (await generateKeyPairSigner()).address;
      const account = await sdk.fetchAccount(createSvmRpc(svm).rpc, settings);
      const ix = await sdk.buildVaultTransaction({
        account, vaultIndex: 0, signers: keys,
        instructions: [{ programAddress: SYSTEM_PROGRAM, accounts: [{ address: vault, role: AccountRole.WRITABLE_SIGNER }, { address: to, role: AccountRole.WRITABLE }], data: concat(u32(2), u64(5_000_000n)) }],
      });
      await execute(svm, payer, [ix], keys);
      assert.equal(svm.getBalance(to), 5_000_000n, show(c));
    }
  });
});
