// Output checks shared by random.test.ts and signers.test.ts: decode the SDK's instruction with the
// test-side decoders and compare it with what the spec says it must mean.
import assert from 'node:assert/strict';

import { type Address, type Instruction, type TransactionSigner, isSignerRole, isWritableRole } from '@solana/kit';

import {
  DISC,
  PROGRAM_ID,
  SYSTEM_PROGRAM,
  applyActions,
  creationComputeUnits,
  decodeCreateData,
  decodeSettingsSyncData,
  decodeVaultSyncData,
} from './program.ts';
import type { AccountSnapshot, SignerEntry } from './sdk.ts';
import { dataOf, entriesToMasks, hex, plain, signerSetKey } from './util.ts';

export interface SetSignersInput {
  account: AccountSnapshot;
  signers: readonly TransactionSigner[];
  newSigners: readonly SignerEntry[];
  newThreshold: number;
  rentPayer: TransactionSigner;
}

/** buildSetSigners postconditions (spec §5): metas, action order, and the post-state. */
export function checkSetSigners(input: SetSignersInput, ix: Instruction): void {
  const p = plain(ix);
  assert.equal(p.programId, PROGRAM_ID);
  assert.deepEqual(p.accounts, [
    { pubkey: input.account.settings, isSigner: false, isWritable: true },
    { pubkey: input.rentPayer.address, isSigner: true, isWritable: true },
    { pubkey: SYSTEM_PROGRAM, isSigner: false, isWritable: false },
    { pubkey: PROGRAM_ID, isSigner: false, isWritable: false },
    ...input.signers.map((s) => ({ pubkey: s.address, isSigner: true, isWritable: false })),
  ]);
  const d = decodeSettingsSyncData(dataOf(ix));
  assert.equal(d.disc, hex(DISC.executeSettingsTransactionSync));
  assert.equal(d.numSigners, input.signers.length);
  const acts = d.actions;
  const first = acts[0];
  const last = acts.at(-1);
  assert.ok(first?.type === 'ChangeThreshold' && first.threshold === 1, 'first action is ChangeThreshold(1)');
  assert.ok(last?.type === 'ChangeThreshold' && last.threshold === input.newThreshold, 'last action is ChangeThreshold(newThreshold)');
  const middle = acts.slice(1, -1);
  assert.ok(middle.every((a) => a.type === 'AddSigner' || a.type === 'RemoveSigner'), 'only Add/Remove between the thresholds');

  const current = new Map(input.account.signers.map((s) => [s.key, entriesToMasks([s])[0]!.mask]));
  const wanted = new Map(entriesToMasks(input.newSigners).map((s) => [s.key, s.mask]));
  const changed = (k: Address) => current.has(k) && wanted.has(k) && current.get(k) !== wanted.get(k);
  const touched = (k: Address) => !current.has(k) || !wanted.has(k) || changed(k);
  for (const a of middle) assert.ok(touched((a as { key: Address }).key), `no action for unchanged key ${(a as { key: Address }).key}`);

  const adds = middle.filter((a) => a.type === 'AddSigner');
  assert.deepEqual(
    adds.map((a) => (a as { key: Address }).key),
    [
      ...input.newSigners.map((s) => s.key).filter((k) => !current.has(k)),
      ...input.newSigners.map((s) => s.key).filter(changed),
    ],
    'AddSigner for each new key, then each changed key, in newSigners order (Grid PATCH order, spec §5 buildSetSigners)',
  );
  const dropped = input.account.signers.map((s) => s.key).filter((k) => !wanted.has(k));
  const removals = middle.flatMap((a, i) => (a.type === 'RemoveSigner' && !wanted.has(a.key) ? [{ key: a.key, i }] : []));
  assert.deepEqual(removals.map((r) => r.key), dropped, 'RemoveSigner of dropped keys in on-chain order');
  const lastAdd = middle.map((a) => a.type).lastIndexOf('AddSigner');
  assert.ok(removals.every((r) => r.i > lastAdd), 'removals of dropped keys come after the adds');
  for (const [i, a] of middle.entries()) {
    if (a.type === 'AddSigner' && changed(a.key)) {
      const r = middle.findIndex((x) => x.type === 'RemoveSigner' && x.key === a.key);
      assert.ok(r >= 0 && r < i, `permission change of ${a.key} is a Remove then an Add`);
    }
  }
  const post = applyActions({ signers: entriesToMasks(input.account.signers), threshold: input.account.threshold }, acts);
  assert.deepEqual(signerSetKey(post.signers), signerSetKey(entriesToMasks(input.newSigners)), 'post-state signer set equals newSigners');
  assert.equal(post.threshold, input.newThreshold);
  assert.equal(d.memo, null);
}

export interface VaultTxInput {
  account: AccountSnapshot;
  vaultIndex: number;
  vault: Address;
  signers: readonly TransactionSigner[];
  instructions: readonly Instruction[];
}
/** buildVaultTransaction postconditions: metas, numSigners, and inner instructions decode back. */
export function checkVaultTransaction(input: VaultTxInput, ix: Instruction): void {
  assert.equal(ix.programAddress, PROGRAM_ID);
  const metas = ix.accounts ?? [];
  assert.equal(metas[0]?.address, input.account.settings);
  assert.ok(metas[0] && !isWritableRole(metas[0].role) && !isSignerRole(metas[0].role), 'settings readonly');
  assert.equal(metas[1]?.address, PROGRAM_ID);
  const remaining = metas.slice(2);
  const d = decodeVaultSyncData(dataOf(ix));
  assert.equal(d.disc, hex(DISC.executeTransactionSync));
  assert.equal(d.accountIndex, input.vaultIndex);
  assert.equal(d.numSigners, input.signers.length);
  assert.deepEqual(remaining.slice(0, input.signers.length).map((m) => m.address), input.signers.map((s) => s.address), 'signers first, in the order given');
  assert.equal(new Set(remaining.map((m) => m.address)).size, remaining.length, 'remaining accounts are deduplicated');
  const signerSet = new Set(input.signers.map((s) => s.address));
  for (const m of remaining) assert.equal(isSignerRole(m.role), signerSet.has(m.address), `signer flag of ${m.address}`);
  assert.ok(!remaining.some((m) => m.address === input.vault && isSignerRole(m.role)), 'vault is not an outer signer');
  const innerWritable = new Set(input.instructions.flatMap((i) => (i.accounts ?? []).filter((a) => isWritableRole(a.role)).map((a) => a.address)));
  for (const m of remaining) assert.equal(isWritableRole(m.role), innerWritable.has(m.address), `writable flag of ${m.address}`);
  assert.equal(d.instructions.length, input.instructions.length);
  d.instructions.forEach((c, k) => {
    const want = input.instructions[k]!;
    assert.equal(remaining[c.programIdIndex]?.address, want.programAddress, `inner ${k} program`);
    assert.deepEqual(c.accountIndexes.map((i) => remaining[i]?.address), (want.accounts ?? []).map((a) => a.address), `inner ${k} accounts`);
    assert.equal(hex(c.data), hex(dataOf(want)), `inner ${k} data`);
  });
}

export interface CreateInput {
  programConfig: { address: Address; treasury: Address; smartAccountIndex: bigint };
  creator: TransactionSigner;
  signers: readonly SignerEntry[];
  threshold: number;
  timeLock: number;
  settingsAuthority: Address | null;
  rentCollector: Address | null;
  memo: string | null;
}
/** ProgramConfig PDA, seeds ["smart_account", "program_config"] (vector pda_program_config). The program only accepts this account. */
const PROGRAM_CONFIG_PDA = 'GmY9kVi3FhrCUn2MJkzzpE6C5618YoHuGsgqHU78cKus' as Address;
/** buildCreateAccount postconditions; `oracle` = [address, bump] of the expected candidates. */
export function checkCreate(
  input: CreateInput,
  res: { instruction: Instruction; candidates: readonly Address[]; computeUnitLimit: number | bigint },
  oracle: readonly (readonly [Address, number])[],
): void {
  const p = plain(res.instruction);
  assert.equal(p.programId, PROGRAM_ID);
  assert.deepEqual(p.accounts, [
    { pubkey: PROGRAM_CONFIG_PDA, isSigner: false, isWritable: true },
    { pubkey: input.programConfig.treasury, isSigner: false, isWritable: true },
    { pubkey: input.creator.address, isSigner: true, isWritable: true },
    { pubkey: SYSTEM_PROGRAM, isSigner: false, isWritable: false },
    { pubkey: PROGRAM_ID, isSigner: false, isWritable: false },
    ...oracle.map(([a]) => ({ pubkey: a, isSigner: false, isWritable: true })),
  ]);
  assert.deepEqual([...res.candidates], oracle.map(([a]) => a));
  assert.equal(Number(res.computeUnitLimit), creationComputeUnits(oracle.map(([, b]) => b)));
  const d = decodeCreateData(dataOf(res.instruction));
  assert.equal(d.disc, hex(DISC.createSmartAccount));
  assert.deepEqual(
    { ...d, disc: undefined, signers: signerSetKey(d.signers) },
    {
      disc: undefined,
      settingsAuthority: input.settingsAuthority,
      threshold: input.threshold,
      signers: signerSetKey(entriesToMasks(input.signers)),
      timeLock: input.timeLock,
      rentCollector: input.rentCollector,
      memo: input.memo,
    },
  );
}
