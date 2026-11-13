// Data-driven checks of the SDK against the byte-exact vectors captured from Grid's Rust builders
// (test/vectors/grid/). Every vector file must be listed in PLAN below, either as an SDK check,
// a harness check (pins the test-side encoders in test/support/program.ts) or a skip with a
// reason, so a new vector cannot be silently ignored.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { type TestContext, describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  type Address,
  type Instruction,
  createKeyPairSignerFromPrivateKeyBytes,
  decompileTransactionMessage,
  getBase64Encoder,
  getCompiledTransactionMessageDecoder,
  getTransactionDecoder,
} from '@solana/kit';

import {
  DEFAULT_PUBKEY,
  PROGRAM_ID,
  type SettingsAction,
  addSpendingLimitAction,
  applyActions,
  changeThresholdAction,
  concat,
  creationComputeUnits,
  decodeSettingsSyncData,
  option,
  programConfigPda,
  reader,
  settingsPda,
  settingsPdaWithBump,
  settingsSyncIx,
  str,
  u8,
  vec,
} from './support/program.ts';
import { accountsRpc } from './support/rpc.ts';
import { type AccountSnapshot, type Permission, type SignerEntry, pdaAddress, sdk } from './support/sdk.ts';
import { type PlainIx, entry, hex, kitIx, noop, plain, show, unhex } from './support/util.ts';

// ---------- loading ----------
type Json = any;
interface Vector {
  case: string;
  input: Json;
  expected: Json;
}
const DIR = fileURLToPath(new URL('./vectors/grid/', import.meta.url));
const FILES = readdirSync(DIR).filter((f) => f.endsWith('.json'));
const VECTORS = new Map<string, Vector>(
  FILES.map((f) => [f.slice(0, -'.json'.length), JSON.parse(readFileSync(DIR + f, 'utf8')) as Vector]),
);
const vector = (name: string): Vector => {
  const v = VECTORS.get(name);
  if (v === undefined) throw new Error(`missing vector ${name}`);
  return v;
};
const KEY3 = '9bydwa51pEgRd576SfrbahGytiZ6Y8CV5Cf77kgsZLc5' as Address;

// ---------- shared mappers ----------
const GRID_PERMS: Record<string, Permission> = { CAN_INITIATE: 'initiate', CAN_VOTE: 'vote', CAN_EXECUTE: 'execute' };
const snapshot = (p: Partial<AccountSnapshot> & Pick<AccountSnapshot, 'settings' | 'signers' | 'threshold'>): AccountSnapshot => ({
  slot: 1n,
  timeLock: 0,
  settingsAuthority: null,
  transactionIndex: 0n,
  ...p,
});
const authorityOf = (a: string | null | undefined) => (a == null || a === DEFAULT_PUBKEY ? null : (a as Address));

/** Snapshot + buildSetSigners arguments from a settings_* vector (request.threshold null = keep current). */
function setSignersArgs(v: Vector, onChainOrder: (s: SignerEntry[]) => SignerEntry[] = (s) => s) {
  const i = v.input;
  const current: SignerEntry[] = i.currentSigners.map((s: Json) => entry(s.address, s.mask));
  const account = snapshot({
    settings: i.settingsAddress,
    threshold: i.currentThreshold,
    signers: onChainOrder(current),
    settingsAuthority: authorityOf(i.settingsAuthority),
  });
  const newSigners: SignerEntry[] =
    i.request.signers == null
      ? current
      : i.request.signers.map((s: Json) => ({ key: s.address, permissions: s.permissions.map((p: string) => GRID_PERMS[p]!) }));
  return {
    account,
    signers: i.transactionSigners.map(noop),
    newSigners,
    newThreshold: i.request.threshold ?? i.currentThreshold,
    rentPayer: noop(i.rentPayer),
  };
}

/** Re-encodes SyncSettingsTransactionArgs from (possibly edited) decoded parts. */
const encodeSettingsData = (d: ReturnType<typeof decodeSettingsSyncData>, actions: readonly SettingsAction[]) =>
  concat(unhex(d.disc), u8(d.numSigners), vec(actions.map((a) => a.raw)), option(d.memo === null ? null : str(d.memo)));

const single = (ixs: Json[]): PlainIx => {
  assert.equal(ixs.length, 1, 'vector has one instruction');
  return ixs[0] as PlainIx;
};

/** Exact comparison of a settings instruction, after an optional edit of the vector's action list. */
async function checkSetSigners(v: Vector, edit?: (a: SettingsAction[]) => SettingsAction[]) {
  const want = single(v.expected.instructions);
  const got = plain(await sdk.buildSetSigners(setSignersArgs(v)));
  if (edit === undefined) {
    assert.deepEqual(got, want);
    return;
  }
  const d = decodeSettingsSyncData(unhex(want.dataHex));
  assert.equal(hex(encodeSettingsData(d, d.actions)), want.dataHex, 'harness re-encodes the vector exactly');
  assert.deepEqual(got.accounts, want.accounts, 'fixed accounts and signer metas');
  assert.equal(got.programId, want.programId);
  assert.equal(got.dataHex, hex(encodeSettingsData(d, edit(d.actions))), 'actions (normalized as documented)');
}

const vaultArgs = (v: Vector) => {
  const i = v.input;
  const members: string[] = [...i.transactionSigners];
  if (!members.includes(KEY3)) members.push(KEY3);
  return {
    account: snapshot({ settings: i.settingsAddress, threshold: i.transactionSigners.length, signers: members.map((k) => entry(k as Address, 7)) }),
    vaultIndex: i.vaultIndex ?? 0,
    signers: i.transactionSigners.map(noop),
  };
};
async function checkVaultTransaction(v: Vector) {
  const ix = await sdk.buildVaultTransaction({ ...vaultArgs(v), instructions: v.input.instructions.map(kitIx) });
  assert.deepEqual(plain(ix), single(v.expected.instructions));
}

const programConfigOf = (pc: Json) => ({
  address: pc.address as Address,
  smartAccountIndex: BigInt(pc.smartAccountIndex),
  treasury: pc.treasury as Address,
  authority: pc.treasury as Address,
  smartAccountCreationFee: 0n,
});
function createArgs(v: Vector, candidateCount: number) {
  const a = v.input.createArgs;
  return {
    programConfig: programConfigOf(v.input.programConfig),
    creator: noop(v.input.creator),
    signers: a.signers.map((s: Json) => entry(s.key, s.mask)),
    threshold: a.threshold,
    timeLock: a.timeLock,
    settingsAuthority: a.settingsAuthority as Address | null,
    rentCollector: a.rentCollector as Address | null,
    memo: a.memo as string | null,
    candidateCount,
  };
}
const smartAccountIx = (ixs: Json[]): PlainIx => {
  const own = ixs.filter((x: Json) => x.programId === PROGRAM_ID);
  assert.equal(own.length, 1);
  return own[0] as PlainIx;
};
async function checkCreate(v: Vector, candidateCount: number) {
  const res = await sdk.buildCreateAccount(createArgs(v, candidateCount));
  assert.deepEqual(plain(res.instruction), smartAccountIx(v.expected.instructions));
  assert.deepEqual([...res.candidates], v.expected.candidates.map((c: Json) => c.settingsAddress));
  assert.equal(Number(res.computeUnitLimit), v.expected.computeUnitLimit);
}

/** buildTransfer arguments for the sync_* transfer vectors. */
function transferArgs(v: Vector, spl: boolean) {
  const ixs = v.input.instructions;
  if (!spl) {
    const d = reader(unhex(ixs[0].dataHex));
    assert.equal(d.u32(), 2, 'system transfer');
    return { ...vaultArgs(v), mint: 'SOL' as const, to: ixs[0].accounts[1].pubkey, amount: d.u64(), decimals: 9, createRecipientAta: false };
  }
  const tc = ixs[1];
  const d = reader(unhex(tc.dataHex));
  assert.equal(d.u8(), 12, 'transferChecked');
  const amount = d.u64();
  return {
    ...vaultArgs(v),
    mint: tc.accounts[1].pubkey as Address,
    to: ixs[0].accounts[2].pubkey as Address, // the ATA's wallet owner
    amount,
    decimals: d.u8(),
    tokenProgram: tc.programId as Address,
    createRecipientAta: true,
  };
}

const decompiledFromBase64 = (b64: string) => {
  const tx = getTransactionDecoder().decode(getBase64Encoder().encode(b64));
  return decompileTransactionMessage(getCompiledTransactionMessageDecoder().decode(tx.messageBytes));
};
const num = (x: number | bigint | undefined) => (x === undefined ? undefined : BigInt(x));
async function checkWrap(v: Vector, form: 'instructions' | 'message') {
  const i = v.input;
  const base = {
    account: snapshot({ settings: i.settingsAddress, threshold: i.transactionSigners.length, signers: [...i.transactionSigners, KEY3].map((k: string) => entry(k as Address, 7)) }),
    vaultIndex: 0,
    signers: i.transactionSigners.map(noop),
  };
  const res =
    form === 'message'
      ? await sdk.wrapTransaction({ ...base, message: decompiledFromBase64(i.transactionBase64) })
      : await sdk.wrapTransaction({ ...base, instructions: v.expected.decompiledInstructions.map(kitIx) });
  assert.deepEqual(plain(res.instruction), single(v.expected.instructions));
  // Expected stripped values come from the vector's own decompiled ComputeBudget instructions.
  let unitLimit: bigint | undefined;
  let unitPrice: bigint | undefined;
  for (const x of v.expected.decompiledInstructions as PlainIx[]) {
    if (x.programId !== 'ComputeBudget111111111111111111111111111111') continue;
    const r = reader(unhex(x.dataHex));
    const tag = r.u8();
    if (tag === 2) unitLimit = BigInt(r.u32());
    if (tag === 3) unitPrice = r.u64();
  }
  assert.equal(num(res.stripped.unitLimit), unitLimit, 'stripped.unitLimit');
  assert.equal(num(res.stripped.unitPrice), unitPrice, 'stripped.unitPrice');
  const fee = unitLimit !== undefined && unitPrice !== undefined ? (unitPrice * unitLimit + 999_999n) / 1_000_000n : undefined;
  assert.equal(res.stripped.priorityFeeLamports, fee, 'stripped.priorityFeeLamports');
}

async function checkPda(got: unknown, v: Vector) {
  const p = got as Address | readonly [Address, number];
  assert.equal(pdaAddress(p), v.expected.address);
  if (typeof p !== 'string') assert.equal(p[1], v.expected.bump);
}

// ---------- the plan: every vector file, accounted for ----------
type Run = (v: Vector, t: TestContext) => Promise<void>;
type Entry = { kind: 'sdk' | 'harness'; run: Run } | { kind: 'skip'; why: string };
const sdkCheck = (run: Run): Entry => ({ kind: 'sdk', run });
const harness = (run: Run): Entry => ({ kind: 'harness', run });
const skip = (why: string): Entry => ({ kind: 'skip', why });
const exactSet = sdkCheck((v) => checkSetSigners(v));
const refusedControlled = sdkCheck(async (v) => {
  const args = setSignersArgs(v);
  assert.notEqual(args.account.settingsAuthority, null);
  const { rejectsWith } = await import('./support/util.ts');
  await rejectsWith(() => sdk.buildSetSigners(args), ['SYNC_NOT_ALLOWED'], v.case);
});
const exactSync = sdkCheck(checkVaultTransaction);
const syncAndTransfer = (spl: boolean) =>
  sdkCheck(async (v) => {
    await checkVaultTransaction(v);
    assert.deepEqual(plain(await sdk.buildTransfer(transferArgs(v, spl))), single(v.expected.instructions), 'buildTransfer');
  });
const settingsPdaCheck = sdkCheck(async (v) => {
  const idx = BigInt(v.input.smartAccountIndex);
  await checkPda(await sdk.deriveSettings(idx), v);
  assert.equal(await settingsPda(idx), v.expected.address, 'harness seeds');
});
const vaultPdaCheck = sdkCheck(async (v) => checkPda(await sdk.deriveVault(v.input.settingsAddress, v.input.accountIndex), v));

const fetchSettingsCheck = sdkCheck(async (v) => {
  const settings = '41gqrPgijYycTaCCzKyLfvqikMEH9fzGCwZYAKQHvMbd';
  const { rpc } = accountsRpc(new Map([[settings, { data: unhex(v.expected.accountDataHex), owner: PROGRAM_ID }]]), 777n);
  const got = await sdk.fetchAccount(rpc, settings as Address);
  const i = v.input;
  assert.deepEqual(
    { ...got, signers: got.signers.map((s) => ({ key: s.key, permissions: [...s.permissions].sort() })) },
    {
      settings,
      slot: 777n,
      threshold: i.threshold,
      timeLock: i.timeLock,
      settingsAuthority: authorityOf(i.settingsAuthority),
      signers: i.signers.map((s: Json) => ({ key: s.key, permissions: [...entry(s.key, s.mask).permissions].sort() })),
      transactionIndex: BigInt(i.transactionIndex),
    },
  );
});

const PLAN: Record<string, Entry> = {
  // --- buildSetSigners (spec §5) ---
  settings_add_and_remove: exactSet,
  settings_add_one_signer: exactSet,
  settings_add_one_signer_explicit_threshold_2: exactSet,
  settings_migrate_privy_turnkey_to_customer_no_threshold: exactSet,
  settings_migrate_privy_turnkey_to_customer_signed_by_both: exactSet,
  settings_migrate_privy_turnkey_to_customer_signed_by_privy: exactSet,
  settings_permission_update_remove_add: exactSet,
  settings_remove_signer_2of3_to_2of2: exactSet,
  // Grid compares permission arrays order-sensitively and emits Remove+Add for a mere reorder.
  // The spec acts only on keys "whose permissions changed" (same mask = unchanged): the SDK must
  // emit the vector's actions minus that spurious pair.
  settings_permission_reorder_spurious_update: sdkCheck((v) =>
    checkSetSigners(v, (a) => a.filter((x) => !((x.type === 'RemoveSigner' || x.type === 'AddSigner') && x.key === '6s335oZfdcJKto9EEvAonj763nTrj6kuf8EAvPwYwYkD'))),
  ),
  // Grid omits ChangeThreshold(1) when no signer changes; spec §5 always emits steps 1 and 4.
  settings_threshold_change_only: sdkCheck((v) =>
    checkSetSigners(v, (a) => [{ type: 'ChangeThreshold', threshold: 1, raw: changeThresholdAction(1) }, ...a]),
  ),
  // SetTimeLock is not an SDK action (§6): compare everything else.
  settings_signers_threshold_and_time_lock: sdkCheck((v) => checkSetSigners(v, (a) => a.filter((x) => x.type !== 'SetTimeLock'))),
  settings_admin_add_signer: refusedControlled,
  settings_admin_clear_admin: refusedControlled,
  settings_admin_threshold_and_new_admin: refusedControlled,
  settings_time_lock_only: skip('SetTimeLock only; the SDK has no time-lock action (§6) and buildSetSigners always emits ChangeThreshold(1)/(n)'),
  settings_no_changes_empty: skip('Grid emits no instruction; the SDK has no "no change" path (buildSetSigners always returns one instruction)'),
  // --- buildRemoveSpendingLimit ---
  settings_remove_spending_limit: sdkCheck(async (v) => {
    const i = v.input;
    const ix = await sdk.buildRemoveSpendingLimit({
      account: snapshot({ settings: i.settingsAddress, threshold: 1, signers: [entry(i.transactionSigners[0], 7)] }),
      signers: i.transactionSigners.map(noop),
      spendingLimit: i.actions[0].spendingLimit,
      rentPayer: noop(i.rentPayer),
    });
    assert.deepEqual(plain(ix), single(v.expected.instructions));
  }),
  // --- buildVaultTransaction / buildTransfer ---
  sync_2_of_3_signers: exactSync,
  sync_member_is_inner_account: exactSync,
  sync_multi_instruction_shared_accounts: exactSync,
  sync_sol_transfer_vault0: syncAndTransfer(false),
  sync_sol_transfer_vault1: syncAndTransfer(false),
  sync_spl_transfer_checked_with_idempotent_ata: syncAndTransfer(true),
  // --- wrapTransaction ---
  wrap_strip_compute_budget_sol_transfer: sdkCheck(async (v) => {
    await checkWrap(v, 'instructions');
    await checkWrap(v, 'message');
  }),
  wrap_cu_limit_only_spl_transfer: sdkCheck(async (v) => {
    await checkWrap(v, 'instructions');
    await checkWrap(v, 'message');
  }),
  // This message's fee payer is the vault itself (as Grid builds it); wrapTransaction ignores the message fee payer.
  wrap_no_compute_budget_two_signers: sdkCheck(async (v) => {
    await checkWrap(v, 'instructions');
    await checkWrap(v, 'message');
  }),
  // --- buildCreateAccount ---
  create_smart_account_1_candidate: sdkCheck((v) => checkCreate(v, 1)),
  create_smart_account_15_candidates: sdkCheck((v) => checkCreate(v, 15)),
  create_smart_account_admin_memo_10_candidates: sdkCheck((v) => checkCreate(v, 10)),
  // Grid caps candidates at 15 (see the vector's note); the SDK honours 25. Compare the 15 Grid
  // candidates and every other byte exactly, then check the 10 extra candidates continue the
  // sequence and the CU limit uses the minimum bump over all 25.
  create_smart_account_25_candidates_requested: sdkCheck(async (v) => {
    const want = smartAccountIx(v.expected.instructions);
    const res = await sdk.buildCreateAccount(createArgs(v, 25));
    const got = plain(res.instruction);
    assert.equal(got.dataHex, want.dataHex);
    assert.deepEqual(got.accounts.slice(0, 20), want.accounts);
    const start = BigInt(v.input.programConfig.smartAccountIndex) + 1n;
    const all = await Promise.all(Array.from({ length: 25 }, (_, k) => settingsPdaWithBump(start + BigInt(k))));
    assert.deepEqual(got.accounts.slice(20), all.slice(15).map(([a]) => ({ pubkey: a, isSigner: false, isWritable: true })));
    assert.deepEqual([...res.candidates], all.map(([a]) => a));
    assert.equal(Number(res.computeUnitLimit), creationComputeUnits(all.map(([, b]) => b)));
  }),
  // --- computeUnitLimit (Grid's calculate_compute_budget) ---
  compute_budget_real_candidates_index_42: sdkCheck(async (v) => {
    const bumps = await Promise.all(Array.from({ length: 15 }, async (_, k) => (await settingsPdaWithBump(42n + BigInt(k)))[1]));
    assert.deepEqual(bumps, v.input.bumps, 'harness: these are the real bumps of candidates 42..56');
    const res = await sdk.buildCreateAccount({ ...createArgs(vector('create_smart_account_15_candidates'), 15) });
    assert.equal(Number(res.computeUnitLimit), v.expected.computeUnits);
  }),
  compute_budget_bump_255: sdkCheck(async (v) => {
    const c = vector('create_smart_account_admin_memo_10_candidates');
    assert.equal(c.expected.candidates[0].bump, 255, 'candidate 1001 has bump 255');
    const res = await sdk.buildCreateAccount(createArgs(c, 1));
    assert.equal(Number(res.computeUnitLimit), v.expected.computeUnits);
  }),
  compute_budget_bump_200: skip('no findable settings index has bump 200 (p≈2^-55); formula is checked on real bumps in random.test.ts'),
  compute_budget_mixed_240: skip('no findable settings index has bump 240; formula is checked on real bumps in random.test.ts'),
  compute_budget_empty: skip('zero candidates is rejected by the SDK (1 ≤ candidateCount, §5)'),
  // --- derive* ---
  pda_settings_index_0: settingsPdaCheck,
  pda_settings_index_1: settingsPdaCheck,
  pda_settings_index_1000: settingsPdaCheck,
  pda_settings_index_18446744073709551616: settingsPdaCheck,
  pda_vault_index_0: vaultPdaCheck,
  pda_vault_index_1: vaultPdaCheck,
  pda_vault_index_255: vaultPdaCheck,
  pda_spending_limit: sdkCheck(async (v) => {
    const { spendingLimitPda } = await import('./support/program.ts');
    assert.equal(await spendingLimitPda(v.input.settingsAddress, v.input.seed), v.expected.address, 'harness seeds');
    assert.equal(typeof sdk.deriveSpendingLimit, 'function', 'deriveSpendingLimit is exported (src/index.ts)');
    await checkPda(await sdk.deriveSpendingLimit(v.input.settingsAddress, v.input.seed), v);
  }),
  pda_program_config: harness(async (v) => assert.equal(await programConfigPda(), v.expected.address)),
  pda_policy_seed_1: skip('policies are out of scope for A (§1, §5)'),
  pda_proposal_index_1: skip('proposals are a non-goal (§1)'),
  pda_transaction_index_1: skip('async transactions/proposals are a non-goal (§1)'),
  pda_transaction_index_1000000: skip('async transactions/proposals are a non-goal (§1)'),
  // --- account decoders through fetch* ---
  borsh_settings_policy_seed_none: fetchSettingsCheck,
  borsh_settings_policy_seed_some: fetchSettingsCheck,
  borsh_spending_limit: sdkCheck(async (v) => {
    const i = v.input;
    const data = unhex(v.expected.accountDataHex);
    const other = Uint8Array.from(data);
    other[8] = (other[8]! + 1) % 256; // a limit of a different settings account
    const junk = Uint8Array.from(data);
    junk[0] = (junk[0]! + 1) % 256; // right settings bytes, wrong discriminator
    const addr = 'DCHptW1j4mHAqU6o3dfJ9aQTaSvNCH2x4WGAkPMSzHjA';
    const { rpc, calls } = accountsRpc(
      new Map([
        [addr, { data, owner: PROGRAM_ID }],
        ['KXG5tZyNivpNxZDprcjfzgm1uQmAJ5KM3T24otXdXu2', { data: other, owner: PROGRAM_ID }],
        ['w2nuVFaXrxHTUxukXnDwwaxU1BANo9dvhdT5ABkVucr', { data: junk, owner: PROGRAM_ID }],
      ]),
    );
    const got = await sdk.fetchSpendingLimits(rpc, i.settings);
    assert.equal(got.length, 1, `exactly the matching limit: ${show(got)}`);
    const sl = got[0]!;
    assert.deepEqual(
      {
        address: sl.address, settings: sl.settings, seed: sl.seed, accountIndex: sl.accountIndex, mint: sl.mint,
        amount: sl.amount, remainingAmount: sl.remainingAmount, lastReset: sl.lastReset, bump: sl.bump,
        signers: [...sl.signers], destinations: [...sl.destinations], expiration: sl.expiration,
      },
      {
        address: addr, settings: i.settings, seed: i.seed, accountIndex: i.accountIndex, mint: i.mint,
        amount: BigInt(i.amount), remainingAmount: BigInt(i.remainingAmount), lastReset: BigInt(i.lastReset), bump: i.bump,
        signers: i.signers, destinations: i.destinations, expiration: BigInt(i.expiration),
      },
    );
    assert.equal(String(sl.period).toLowerCase(), String(i.period).toLowerCase(), `period ${show(sl.period)}`);
    const gpa = calls.filter((c) => c.method === 'getProgramAccounts');
    assert.ok(gpa.length >= 1 && gpa.every((c) => c.args[0] === PROGRAM_ID), 'getProgramAccounts on the program');
    const filters = show(gpa[0]!.args[1]);
    assert.match(filters, /memcmp/, `filtered with memcmp: ${filters}`);
  }),
  borsh_program_config: harness(async (v) => {
    const r = reader(unhex(v.expected.accountDataHex));
    assert.equal(hex(r.take(8)), v.expected.discriminatorHex);
    assert.equal(r.u128(), BigInt(v.input.smartAccountIndex));
    assert.equal(r.pubkey(), v.input.authority);
    assert.equal(r.u64(), BigInt(v.input.smartAccountCreationFee));
    assert.equal(r.pubkey(), v.input.treasury);
  }),
  // --- harness pins ---
  keys_deterministic: harness(async (v) => {
    for (const k of v.expected.keys as Json[]) {
      const seed = createHash('sha256').update(k.seedPreimage).digest();
      assert.equal(hex(seed), k.seedHex);
      assert.equal((await createKeyPairSignerFromPrivateKeyBytes(new Uint8Array(seed))).address, k.pubkey);
    }
  }),
  spending_limit_standing_order_daily_2_signers: harness(checkAddSpendingLimitEncoder),
  spending_limit_standing_order_monthly: harness(checkAddSpendingLimitEncoder),
  kms_privy_payload: skip('KMS payloads belong to Part B (@sqds/grid/migration), not this package'),
  kms_turnkey_payload: skip('KMS payloads belong to Part B (@sqds/grid/migration), not this package'),
};

/** Pins the test-only AddSpendingLimit encoder (used in svm.test.ts) to Grid's bytes. */
async function checkAddSpendingLimitEncoder(v: Vector) {
  const i = v.input;
  const periods: Record<string, number> = { OneTime: 0, Day: 1, Week: 2, Month: 3 };
  const action = addSpendingLimitAction({
    seed: i.seed, accountIndex: 0, mint: i.mint, amount: BigInt(i.amount), period: periods[i.period]!,
    signers: i.spendingLimitSigners, destinations: i.destinations, expiration: BigInt(i.expiration),
  });
  assert.equal(hex(action), v.expected.actions[0].borshHex);
  const ix: Instruction = settingsSyncIx({
    settings: i.settingsAddress,
    rentPayer: noop(i.payer),
    signers: i.transactionSigners.map(noop),
    actions: [action],
    remaining: [v.expected.spendingLimitAddress],
  });
  assert.deepEqual(plain(ix), single(v.expected.instructions));
}

describe('Grid vectors: accounting', () => {
  it('every vector file is covered or explicitly skipped, and the plan names no missing file', () => {
    assert.deepEqual([...VECTORS.keys()].filter((k) => !(k in PLAN)), [], 'vectors with no plan entry');
    assert.deepEqual(Object.keys(PLAN).filter((k) => !VECTORS.has(k)), [], 'plan entries with no vector');
    for (const [k, v] of VECTORS) assert.equal(v.case, k, 'file name matches case');
  });
  it('the vector files match SHA256SUMS (reference data untouched)', () => {
    const sums = readFileSync(DIR + 'SHA256SUMS', 'utf8').trim().split('\n').map((l) => l.split(/\s+/));
    assert.equal(sums.length, FILES.length);
    for (const [sum, f] of sums) assert.equal(createHash('sha256').update(readFileSync(DIR + f!)).digest('hex'), sum, f);
  });
  it('the harness Settings/SpendingLimit discriminators match the vectors', async () => {
    const { DISC } = await import('./support/program.ts');
    assert.equal(hex(DISC.settings), vector('borsh_settings_policy_seed_none').expected.discriminatorHex);
    assert.equal(hex(DISC.spendingLimit), vector('borsh_spending_limit').expected.discriminatorHex);
    assert.equal(hex(DISC.programConfig), vector('borsh_program_config').expected.discriminatorHex);
  });
});

describe('Grid vectors', () => {
  for (const [name, plan] of Object.entries(PLAN)) {
    if (plan.kind === 'skip') {
      it(`${name} [skipped]`, { skip: plan.why }, () => {});
      continue;
    }
    it(`${name} [${plan.kind}]`, (t) => plan.run(vector(name), t));
  }
});

describe('Grid vectors: documented normalizations', () => {
  // Grid orders removals by its database; the spec by on-chain signer order. With the snapshot's
  // signers reversed, the SDK must emit the same actions as a set, removals in the snapshot's order.
  it('removals follow the snapshot (on-chain) order, not Grid DB order', async () => {
    const v = vector('settings_migrate_privy_turnkey_to_customer_signed_by_both');
    const want = single(v.expected.instructions);
    const args = setSignersArgs(v, (s) => [...s].reverse());
    const got = plain(await sdk.buildSetSigners(args));
    assert.deepEqual(got.accounts, want.accounts);
    const g = decodeSettingsSyncData(unhex(got.dataHex));
    const w = decodeSettingsSyncData(unhex(want.dataHex));
    assert.deepEqual([g.disc, g.numSigners, g.memo], [w.disc, w.numSigners, w.memo]);
    assert.deepEqual(g.actions.map((a) => hex(a.raw)).sort(), w.actions.map((a) => hex(a.raw)).sort(), 'same actions as a set');
    assert.deepEqual(
      g.actions.filter((a) => a.type === 'RemoveSigner').map((a) => (a as { key: string }).key),
      args.account.signers.map((s) => s.key),
      'removals in snapshot order',
    );
    const post = applyActions(
      { signers: args.account.signers.map((s) => ({ key: s.key, mask: 7 })), threshold: 1 },
      g.actions,
    );
    assert.deepEqual(post, { signers: [{ key: KEY3, mask: 7 }], threshold: 1 });
  });
  it('buildCreateAccount defaults to 25 candidates', async () => {
    const c = vector('create_smart_account_1_candidate');
    const { candidateCount: _omit, ...args } = createArgs(c, 1);
    const res = await sdk.buildCreateAccount(args);
    assert.equal(res.candidates.length, 25);
    assert.equal(plain(res.instruction).accounts.length, 5 + 25);
  });
});
