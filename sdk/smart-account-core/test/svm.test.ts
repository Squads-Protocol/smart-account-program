// End-to-end on the pinned mainnet binary in LiteSVM: SDK builders -> SDK `send` (v1) over a
// minimal RPC adapter -> post-state read back with the test-side decoders (and the SDK's fetch*).
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  AccountRole,
  type Address,
  type Instruction,
  type KeyPairSigner,
  address,
  appendTransactionMessageInstructions,
  blockhash,
  createTransactionMessage,
  generateKeyPairSigner,
  lamports,
  pipe,
  setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
} from '@solana/kit';

import {
  COMPUTE_BUDGET_PROGRAM,
  PROGRAM_ID,
  SYSTEM_PROGRAM,
  TOKEN_2022_PROGRAM,
  TOKEN_PROGRAM,
  addSpendingLimitAction,
  ataPda,
  concat,
  createSmartAccountIx,
  decodeSettingsAccount,
  mintData,
  settingsPda,
  settingsSyncIx,
  spendingLimitPda,
  tokenAccountData,
  tokenAmountOf,
  u32,
  u64,
  u8,
  vaultPda,
} from './support/program.ts';
import { type SendOutcome, sdk } from './support/sdk.ts';
import { createSvmRpc, execute, readProgramConfig, setRaw, setupSvm } from './support/svm.ts';
import { ALL, entry, permsToMask, show, signerSetKey } from './support/util.ts';

const SOL = 1_000_000_000n;

async function env() {
  const { svm, programConfig } = await setupSvm();
  const net = createSvmRpc(svm);
  const payer = await generateKeyPairSigner();
  svm.airdrop(payer.address, lamports(100n * SOL));
  /** Creates an account with the test-only encoder (independent of the SDK). */
  const createDirect = async (members: { key: Address; mask: number }[], threshold: number) => {
    const pc = readProgramConfig(svm, programConfig.address);
    const settings = await settingsPda(pc.smartAccountIndex + 1n);
    await execute(svm, payer, [
      createSmartAccountIx({ programConfig: pc.address, treasury: pc.treasury, creator: payer, signers: members, threshold, candidates: [settings] }),
    ]);
    return settings;
  };
  const settingsOf = (a: Address) => {
    const acc = svm.getAccount(a);
    assert.ok(acc.exists, `settings ${a} exists`);
    return decodeSettingsAccount(acc.data);
  };
  const bal = (a: Address) => svm.getBalance(a) ?? 0n;
  const send = async (instructions: Instruction[], signers: KeyPairSigner[], computeUnitLimit?: number) => {
    const out = await sdk.send(net.rpc, {
      instructions,
      feePayer: payer,
      signers: [payer, ...signers],
      ...(computeUnitLimit === undefined ? {} : { computeUnitLimit }),
    });
    return out;
  };
  return { svm, programConfig, payer, ...net, createDirect, settingsOf, bal, send };
}
const keys = (n: number) => Promise.all(Array.from({ length: n }, () => generateKeyPairSigner()));
function confirmed(out: SendOutcome, label: string): asserts out is Extract<SendOutcome, { status: 'confirmed' }> {
  assert.equal(out.status, 'confirmed', `${label}: ${show(out)}`);
}
/** Every transaction the SDK sent is a v1 transaction and set its config (CU + loaded-accounts). */
function assertAllV1(history: ReturnType<typeof createSvmRpc>['history']) {
  assert.ok(history.size > 0);
  for (const [sig, r] of history) assert.equal(r.wire[0], 0x81, `tx ${sig} is v1`);
}

describe('LiteSVM harness (test-only encoders on the mainnet binary)', () => {
  it('creates an account and adds a spending limit with the test-only encoders', async () => {
    const e = await env();
    const [m] = await keys(1);
    const settings = await e.createDirect([{ key: m!.address, mask: 7 }], 1);
    assert.deepEqual(e.settingsOf(settings).signers, [{ key: m!.address, mask: 7 }]);
    const seed = (await generateKeyPairSigner()).address;
    const sl = await spendingLimitPda(settings, seed);
    await execute(e.svm, e.payer, [
      settingsSyncIx({
        settings,
        rentPayer: e.payer,
        signers: [m!],
        actions: [addSpendingLimitAction({ seed, accountIndex: 0, mint: SYSTEM_PROGRAM, amount: 5n, period: 1, signers: [m!.address], destinations: [], expiration: 1n << 62n })],
        remaining: [sl],
      }),
    ]);
    assert.ok(e.svm.getAccount(sl).exists);
  });
});

describe('SDK on LiteSVM (mainnet binary, v1 transactions via send)', () => {
  it('buildCreateAccount (25 candidates) + send + resolveCreatedAccount under a concurrent creator', async () => {
    const e = await env();
    const [a, b, rival] = await keys(3);
    const built = await sdk.buildCreateAccount({
      programConfig: e.programConfig,
      creator: e.payer,
      signers: [entry(a!.address, 7), entry(b!.address, 7)],
      threshold: 1,
      timeLock: 0,
      settingsAuthority: null,
      rentCollector: null,
      memo: null,
    });
    assert.equal(built.candidates.length, 25);
    assert.equal(built.candidates[0], await settingsPda(e.programConfig.smartAccountIndex + 1n));
    // Another creator takes index+1 first, so this transaction lands on the second candidate.
    const taken = await e.createDirect([{ key: rival!.address, mask: 7 }], 1);
    assert.equal(taken, built.candidates[0]);
    const out = await e.send([built.instruction], [], Number(built.computeUnitLimit));
    confirmed(out, 'create');
    const created = await sdk.resolveCreatedAccount(e.rpc, out.signature, built.candidates);
    assert.equal(created, built.candidates[1], 'the candidate this transaction funded, not the first initialized one');
    assert.deepEqual(signerSetKey(e.settingsOf(created).signers), signerSetKey([{ key: a!.address, mask: 7 }, { key: b!.address, mask: 7 }]));
    const snap = await sdk.fetchAccount(e.rpc, created);
    assert.equal(snap.settings, created);
    assert.equal(snap.threshold, 1);
    assert.equal(snap.timeLock, 0);
    assert.equal(snap.settingsAuthority, null);
    assert.deepEqual(signerSetKey(snap.signers.map((s) => ({ key: s.key, mask: permsToMask(s.permissions) }))), signerSetKey([{ key: a!.address, mask: 7 }, { key: b!.address, mask: 7 }]));
    for (const c of e.calls.filter((x) => x.method === 'getTransaction')) {
      assert.equal((c.args[1] as { maxSupportedTransactionVersion?: number }).maxSupportedTransactionVersion, 1);
    }
    // Safeguard removed: "first initialized candidate" would name the rival's account.
    const naive = built.candidates.find((c) => e.svm.getAccount(c).exists);
    assert.notEqual(naive, created, 'the scenario discriminates a naive resolver');
    assertAllV1(e.history);
  });

  it('buildCreateAccount without computeUnitLimit (send simulates) creates the account', async () => {
    const e = await env();
    const [a] = await keys(1);
    const built = await sdk.buildCreateAccount({
      programConfig: e.programConfig, creator: e.payer, signers: [entry(a!.address, 7)], threshold: 1, timeLock: 0,
      settingsAuthority: null, rentCollector: null, memo: 'hello', candidateCount: 3,
    });
    const out = await e.send([built.instruction], []);
    confirmed(out, 'create');
    assert.equal(await sdk.resolveCreatedAccount(e.rpc, out.signature, built.candidates), built.candidates[0]);
    assertAllV1(e.history);
  });

  for (const vaultIndex of [0, 1]) {
    it(`SOL transfer from vault ${vaultIndex}`, async () => {
      const e = await env();
      const [a, b] = await keys(2);
      const settings = await e.createDirect([{ key: a!.address, mask: 7 }, { key: b!.address, mask: 7 }], 1);
      const vault = await vaultPda(settings, vaultIndex);
      e.svm.airdrop(vault, lamports(2n * SOL));
      const to = (await generateKeyPairSigner()).address;
      const account = await sdk.fetchAccount(e.rpc, settings);
      const amount = 123_456_789n;
      const before = e.bal(vault);
      const ix = await sdk.buildTransfer({ account, vaultIndex, signers: [a!], mint: 'SOL', to, amount, decimals: 9, createRecipientAta: false });
      confirmed(await e.send([ix], [a!]), 'transfer');
      assert.equal(e.bal(to), amount);
      assert.equal(e.bal(vault), before - amount, 'vault debited exactly (fees paid by the fee payer)');
      assertAllV1(e.history);
    });
  }

  for (const [vaultIndex, tokenProgram] of [[0, TOKEN_PROGRAM], [1, TOKEN_2022_PROGRAM]] as const) {
    it(`SPL transferChecked with recipient ATA creation from vault ${vaultIndex} (${tokenProgram.slice(0, 8)})`, async () => {
      const e = await env();
      const [a] = await keys(1);
      const settings = await e.createDirect([{ key: a!.address, mask: 7 }], 1);
      const vault = await vaultPda(settings, vaultIndex);
      e.svm.airdrop(vault, lamports(SOL));
      const mint = (await generateKeyPairSigner()).address;
      setRaw(e.svm, mint, tokenProgram, mintData(6));
      const source = await ataPda(vault, mint, tokenProgram);
      setRaw(e.svm, source, tokenProgram, tokenAccountData(mint, vault, 5_000_000n));
      const recipient = (await generateKeyPairSigner()).address;
      const dest = await ataPda(recipient, mint, tokenProgram);
      assert.equal(e.svm.getAccount(dest).exists, false);
      const account = await sdk.fetchAccount(e.rpc, settings);
      const vaultBefore = e.bal(vault);
      const ix = await sdk.buildTransfer({
        account, vaultIndex, signers: [a!], mint, to: recipient, amount: 1_500_000n, decimals: 6, tokenProgram, createRecipientAta: true,
      });
      confirmed(await e.send([ix], [a!]), 'spl transfer');
      const d = e.svm.getAccount(dest);
      assert.ok(d.exists && d.programAddress === tokenProgram, 'recipient ATA created');
      assert.equal(tokenAmountOf(d.data), 1_500_000n);
      const s = e.svm.getAccount(source);
      assert.ok(s.exists);
      assert.equal(tokenAmountOf(s.data), 3_500_000n);
      assert.equal(vaultBefore - e.bal(vault), d.lamports, 'the vault paid the ATA rent');
      assertAllV1(e.history);
    });
  }

  it('wrapTransaction of a multi-instruction message executes it from the vault', async () => {
    const e = await env();
    const [a] = await keys(1);
    const settings = await e.createDirect([{ key: a!.address, mask: 7 }], 1);
    const vault = await vaultPda(settings, 0);
    e.svm.airdrop(vault, lamports(SOL));
    const mint = (await generateKeyPairSigner()).address;
    setRaw(e.svm, mint, TOKEN_PROGRAM, mintData(2));
    const src = await ataPda(vault, mint);
    setRaw(e.svm, src, TOKEN_PROGRAM, tokenAccountData(mint, vault, 1_000n));
    const owner = (await generateKeyPairSigner()).address;
    const dst = await ataPda(owner, mint);
    setRaw(e.svm, dst, TOKEN_PROGRAM, tokenAccountData(mint, owner, 0n));
    const [d1, d2] = [(await generateKeyPairSigner()).address, (await generateKeyPairSigner()).address];
    const vaultSigner = { address: vault, role: AccountRole.WRITABLE_SIGNER };
    const sysTransfer = (to: Address, n: bigint): Instruction => ({
      programAddress: SYSTEM_PROGRAM,
      accounts: [vaultSigner, { address: to, role: AccountRole.WRITABLE }],
      data: concat(u32(2), u64(n)),
    });
    const message = pipe(
      createTransactionMessage({ version: 0 }),
      (m) => setTransactionMessageFeePayer(address('F6ptqu6PKt56WLT3MmqayimN4HL7b8PARFRwWQXi4Quk'), m),
      (m) => setTransactionMessageLifetimeUsingBlockhash({ blockhash: blockhash('CmpNeggWJ4JaWJeJ8YKN1Zypmk7uvQq3PECGUCAEMbky'), lastValidBlockHeight: 9n }, m),
      (m) =>
        appendTransactionMessageInstructions(
          [
            { programAddress: COMPUTE_BUDGET_PROGRAM, data: concat(u8(2), u32(300_000)) },
            sysTransfer(d1, 1_000_000n),
            { programAddress: COMPUTE_BUDGET_PROGRAM, data: concat(u8(3), u64(5n)) },
            sysTransfer(d2, 2_000_000n),
            {
              programAddress: TOKEN_PROGRAM,
              accounts: [
                { address: src, role: AccountRole.WRITABLE },
                { address: mint, role: AccountRole.READONLY },
                { address: dst, role: AccountRole.WRITABLE },
                { address: vault, role: AccountRole.READONLY_SIGNER },
              ],
              data: concat(u8(12), u64(250n), u8(2)),
            },
          ],
          m,
        ),
    );
    const account = await sdk.fetchAccount(e.rpc, settings);
    const { instruction, stripped } = await sdk.wrapTransaction({ account, vaultIndex: 0, signers: [a!], message });
    assert.equal(BigInt(stripped.unitLimit ?? -1), 300_000n);
    assert.equal(BigInt(stripped.unitPrice ?? -1), 5n);
    assert.ok(!(instruction.accounts ?? []).some((x) => x.address === COMPUTE_BUDGET_PROGRAM), 'no ComputeBudget inside');
    confirmed(await e.send([instruction], [a!]), 'wrapped');
    assert.equal(e.bal(d1), 1_000_000n);
    assert.equal(e.bal(d2), 2_000_000n);
    const dAcc = e.svm.getAccount(dst);
    assert.ok(dAcc.exists);
    assert.equal(tokenAmountOf(dAcc.data), 250n);
  });

  it('buildSetSigners on the Grid email-account shape -> {customer}', async () => {
    const e = await env();
    const [privy, turnkey, customer] = await keys(3);
    const settings = await e.createDirect([{ key: privy!.address, mask: 7 }, { key: turnkey!.address, mask: 7 }], 1);
    const account = await sdk.fetchAccount(e.rpc, settings);
    assert.equal(account.settingsAuthority, null);
    const ix = await sdk.buildSetSigners({
      account, signers: [privy!], newSigners: [entry(customer!.address, 7)], newThreshold: 1, rentPayer: e.payer,
    });
    confirmed(await e.send([ix], [privy!]), 'set signers');
    const after = e.settingsOf(settings);
    assert.deepEqual(after.signers, [{ key: customer!.address, mask: 7 }]);
    assert.equal(after.threshold, 1);
    const snap = await sdk.fetchAccount(e.rpc, settings);
    assert.deepEqual(snap.signers.map((s) => ({ key: s.key, permissions: [...s.permissions].sort() })), [{ key: customer!.address, permissions: [...ALL].sort() }]);
    assert.ok(snap.slot >= account.slot);
    // The customer key alone now controls the vault.
    const vault = await vaultPda(settings, 0);
    e.svm.airdrop(vault, lamports(SOL));
    const to = (await generateKeyPairSigner()).address;
    const t = await sdk.buildTransfer({ account: snap, vaultIndex: 0, signers: [customer!], mint: 'SOL', to, amount: 7_000_000n, decimals: 9, createRecipientAta: false });
    confirmed(await e.send([t], [customer!]), 'customer transfer');
    assert.equal(e.bal(to), 7_000_000n);
  });

  it('fetchSpendingLimits + buildRemoveSpendingLimit on a standing-order-shaped limit', async () => {
    const e = await env();
    const [member, standingOrderKey, rentPayer] = await keys(3);
    e.svm.airdrop(rentPayer!.address, lamports(SOL));
    const settings = await e.createDirect([{ key: member!.address, mask: 7 }], 1);
    const otherSettings = await e.createDirect([{ key: member!.address, mask: 7 }], 1);
    const mint = (await generateKeyPairSigner()).address;
    const destination = (await generateKeyPairSigner()).address;
    const addLimit = async (s: Address) => {
      const seed = (await generateKeyPairSigner()).address;
      const sl = await spendingLimitPda(s, seed);
      await execute(e.svm, e.payer, [
        settingsSyncIx({
          settings: s, rentPayer: e.payer, signers: [member!], remaining: [sl],
          actions: [addSpendingLimitAction({ seed, accountIndex: 0, mint, amount: 1_000_000n, period: 1, signers: [standingOrderKey!.address], destinations: [destination], expiration: 1_800_000_000n })],
        }),
      ]);
      return { seed, sl };
    };
    const mine = await addLimit(settings);
    const theirs = await addLimit(otherSettings);
    const limits = await sdk.fetchSpendingLimits(e.rpc, settings);
    assert.equal(limits.length, 1, show(limits));
    const l = limits[0]!;
    assert.deepEqual(
      { address: l.address, settings: l.settings, seed: l.seed, accountIndex: l.accountIndex, mint: l.mint, amount: l.amount, signers: [...l.signers], destinations: [...l.destinations], expiration: l.expiration },
      { address: mine.sl, settings, seed: mine.seed, accountIndex: 0, mint, amount: 1_000_000n, signers: [standingOrderKey!.address], destinations: [destination], expiration: 1_800_000_000n },
    );
    const account = await sdk.fetchAccount(e.rpc, settings);
    const slLamports = e.bal(mine.sl);
    const rentBefore = e.bal(rentPayer!.address);
    const ix = await sdk.buildRemoveSpendingLimit({ account, signers: [member!], spendingLimit: l.address, rentPayer: rentPayer! });
    confirmed(await e.send([ix], [member!, rentPayer!]), 'remove limit');
    assert.equal(e.svm.getAccount(mine.sl).exists, false, 'limit closed');
    assert.equal(e.bal(rentPayer!.address) - rentBefore, slLamports, 'rent went to rentPayer');
    assert.deepEqual(await sdk.fetchSpendingLimits(e.rpc, settings), []);
    assert.ok(e.svm.getAccount(theirs.sl).exists, "another account's limit is untouched");
    assert.equal((await sdk.fetchSpendingLimits(e.rpc, otherSettings)).length, 1);
  });

  it('fetchBalances: lamports plus SPL Token and Token-2022 accounts owned by the vault', async () => {
    const e = await env();
    const [a] = await keys(1);
    const settings = await e.createDirect([{ key: a!.address, mask: 7 }], 1);
    const vault = await vaultPda(settings, 0);
    e.svm.airdrop(vault, lamports(3n * SOL));
    const [m1, m2, m3] = [(await generateKeyPairSigner()).address, (await generateKeyPairSigner()).address, (await generateKeyPairSigner()).address];
    setRaw(e.svm, m1, TOKEN_PROGRAM, mintData(6));
    setRaw(e.svm, m2, TOKEN_2022_PROGRAM, mintData(9));
    setRaw(e.svm, m3, TOKEN_PROGRAM, mintData(0));
    setRaw(e.svm, await ataPda(vault, m1), TOKEN_PROGRAM, tokenAccountData(m1, vault, 10n));
    setRaw(e.svm, (await generateKeyPairSigner()).address, TOKEN_2022_PROGRAM, tokenAccountData(m2, vault, 2n ** 63n + 7n)); // not an ATA
    setRaw(e.svm, (await generateKeyPairSigner()).address, TOKEN_PROGRAM, tokenAccountData(m3, a!.address, 99n)); // someone else's
    const b = await sdk.fetchBalances(e.rpc, vault);
    assert.equal(b.lamports, e.bal(vault));
    const byMint = [...b.tokens].sort((x, y) => (x.mint < y.mint ? -1 : 1));
    const want = [
      { mint: m1, tokenProgram: TOKEN_PROGRAM, amount: 10n, decimals: 6 },
      { mint: m2, tokenProgram: TOKEN_2022_PROGRAM, amount: 2n ** 63n + 7n, decimals: 9 },
    ].sort((x, y) => (x.mint < y.mint ? -1 : 1));
    assert.deepEqual(byMint.map((t) => ({ mint: t.mint, tokenProgram: t.tokenProgram, amount: t.amount, decimals: t.decimals })), want);
  });

  it('every fetch* / resolveCreatedAccount read passes commitment: confirmed (a custom transport must not default to finalized)', async () => {
    const e = await env();
    const [a] = await keys(1);
    const built = await sdk.buildCreateAccount({
      programConfig: e.programConfig, creator: e.payer, signers: [entry(a!.address, 7)], threshold: 1, timeLock: 0,
      settingsAuthority: null, rentCollector: null, memo: null, candidateCount: 2,
    });
    const out = await e.send([built.instruction], [], Number(built.computeUnitLimit));
    confirmed(out, 'create');
    const settings = await sdk.resolveCreatedAccount(e.rpc, out.signature, built.candidates);
    const vault = await vaultPda(settings, 0);
    const mint = (await generateKeyPairSigner()).address;
    setRaw(e.svm, mint, TOKEN_PROGRAM, mintData(6));
    setRaw(e.svm, await ataPda(vault, mint), TOKEN_PROGRAM, tokenAccountData(mint, vault, 1n));
    const from = e.calls.length;
    await sdk.fetchAccount(e.rpc, settings);
    await sdk.fetchSpendingLimits(e.rpc, settings);
    await sdk.fetchBalances(e.rpc, vault);
    await sdk.resolveCreatedAccount(e.rpc, out.signature, built.candidates);
    const reads = e.calls.slice(from);
    const configOf = (c: { method: string; args: unknown[] }) => (c.method === 'getTokenAccountsByOwner' ? c.args[2] : c.args[1]);
    const check = (calls: typeof reads) => {
      for (const c of calls) assert.equal((configOf(c) as { commitment?: string } | undefined)?.commitment, 'confirmed', `${c.method} commitment`);
    };
    for (const m of ['getAccountInfo', 'getProgramAccounts', 'getBalance', 'getTokenAccountsByOwner', 'getMultipleAccounts', 'getTransaction']) {
      assert.ok(reads.some((c) => c.method === m), `${m} was called`);
    }
    check(reads);
    // Safeguard removed: a call without an explicit commitment is caught.
    assert.throws(() => check([{ method: 'getBalance', args: [vault] }]), /getBalance commitment/);
  });

  it('the program id the SDK targets is the mainnet program', async () => {
    const e = await env();
    const [a] = await keys(1);
    const built = await sdk.buildCreateAccount({
      programConfig: e.programConfig, creator: e.payer, signers: [entry(a!.address, 7)], threshold: 1, timeLock: 0,
      settingsAuthority: null, rentCollector: null, memo: null, candidateCount: 1,
    });
    assert.equal(built.instruction.programAddress, PROGRAM_ID);
  });
});
