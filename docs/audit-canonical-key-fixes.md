# Audit Fix: Canonical Key Usage

## Background

Session keys and external signers have a **canonical key** (the parent signer's `key_id`) that differs from the raw account key used in the transaction. `verify_signer` already returns this canonical key, but several instruction handlers ignore the return value and store/compare the raw `creator.key()` or `signer.key()` instead.

This causes two classes of problems:
1. **Locked state** — if a session key creates state that later requires the same key to operate on it, expiration/revocation of that session key permanently locks the state (rent trapped, operations blocked).
2. **Inconsistent indexing** — events log the session key pubkey instead of the canonical parent key, making off-chain indexing unreliable.

`verify_signer` signature for reference:
```rust
fn verify_signer(...) -> Result<Pubkey>  // Returns canonical_key
```

---

## Issue 1: TransactionBuffer creator uses raw key [HIGH]

### Problem
`transaction_buffer_create` stores `creator.key()` (which may be a session key) as the buffer's `creator` field. Downstream instructions (`close`, `extend`, `create_from_buffer`) check `transaction_buffer.creator == creator.key()` and require `creator` to sign. If the session key expires or is revoked, the buffer is permanently locked and rent is trapped.

Additionally, `creator.key()` is baked into the PDA seeds for the buffer account.

### Affected files
- `transaction_buffer_create.rs:132` — stores `creator.key()`
- `transaction_buffer_create.rs:42` — PDA seed uses `creator.key()`
- `transaction_buffer_close.rs:20,27` — checks and PDA seed use `creator.key()`
- `transaction_buffer_extend.rs:28,33` — checks and PDA seed use `creator.key()`
- `transaction_create_from_buffer.rs:19,24` — checks and PDA seed use `creator.key()`

### Fix

**Option A: Store canonical key, derive PDA with canonical key**

The cleanest approach. The `creator` account in the instruction still receives the session key (or external signer account), but after `verify_signer` succeeds and returns the canonical key, we use that canonical key for:
1. Storing as `transaction_buffer.creator`
2. PDA derivation (passed as a separate arg or resolved on-chain)

This requires the caller (SDK) to know the canonical key upfront for PDA derivation. Since the SDK already has access to the signer list, this is feasible.

Changes:
- `transaction_buffer_create`: capture `canonical_key` from `verify_signer` return value, store it as `creator`, change PDA seeds to use a `canonical_creator` key passed in args or derived.
- `transaction_buffer_close`, `extend`, `create_from_buffer`: the `creator` account becomes the canonical signer (parent key), not the session key. PDA seeds match. The `creator` check becomes: verify that the signer (native, session key, or external) resolves to the stored canonical `creator`.
- `transaction_buffer_close` currently requires `creator: Signer<'info>` — this needs to change to `AccountInfo<'info>` with `verify_signer` (same pattern as create/extend).

**Option B: Allow any signer that resolves to the same canonical key to operate on the buffer**

Keep PDA seeds using `creator.key()` (the session key). Store canonical key as `creator`. On close/extend/create_from_buffer, instead of checking `transaction_buffer.creator == creator.key()`, verify the incoming signer's canonical key matches the stored canonical key.

Problem: PDA derivation still requires knowing the original session key pubkey, which may no longer be discoverable after revocation. This makes the buffer inaccessible even if the parent signer is valid.

**Recommendation: Option A** — use canonical key in PDA seeds. This is a breaking change to the PDA derivation but it's the only approach that fully solves the problem.

---

## Issue 2: Batch creator uses raw key [HIGH]

### Problem
`batch_create` stores `creator.key()` as `batch.creator`. `batch_add_transaction` checks `signer.key() == batch.creator`. If a session key creates the batch and then expires before all transactions are added, no more transactions can be added.

### Affected files
- `batch_create.rs:118` — stores `creator.key()`
- `batch_add_transaction.rs:111` — checks `signer.key() == batch.creator`

### Fix
1. `batch_create.rs`: capture `canonical_key` from `verify_signer` return, store as `batch.creator`
2. `batch_add_transaction.rs`: after `verify_signer` returns the canonical key, compare that against `batch.creator` instead of raw `signer.key()`

No PDA seed issue here — the batch PDA is seeded by `transaction_index`, not `creator`.

```rust
// batch_create.rs — in create_batch_inner:
let canonical_key = settings.resolve_canonical_key(creator.key(), creator.is_signer)?;
batch.creator = canonical_key;

// batch_add_transaction.rs — in validate:
let canonical_key = settings.verify_signer(signer, ...)?;
require!(
    canonical_key == batch.creator,
    SmartAccountError::Unauthorized
);
```

---

## Issue 3: increment_account_index_v2 doesn't support session keys [MEDIUM]

### Problem
`increment_account_index_v2` uses `is_signer_v2(signer_key)` directly, which only matches direct signers. It does not call `find_signer_by_session_key` and does not call `verify_signer`. A session key calling this instruction gets `NotASigner`.

### Affected files
- `increment_account_index.rs:117-119` (V2 path)

### Fix
Replace the manual `is_signer_v2` + permission check + external verification logic with `settings.verify_signer(...)`, which handles all three signer types (native, session key, external) uniformly. This also handles nonce/counter updates.

The V2 handler currently has ~50 lines of manual verification that duplicates what `verify_signer` does. Refactoring to use `verify_signer` (with a permission check after) would fix the session key gap and reduce code duplication.

Note: the permission check here is `Initiate OR Vote OR Execute`, which is not a single permission. `verify_signer` only supports `Option<Permission>` (single). Two options:
- Pass `None` for permission to `verify_signer`, then check the OR permission manually (same as `proposal_create` does).
- Or keep the manual approach but add `find_signer_by_session_key` fallback.

---

## Issue 4: Event logging uses raw keys [LOW]

### Problem
Several instructions log the raw `creator.key()` or `signer.key()` in events instead of the canonical key. This causes inconsistent off-chain indexing — the same logical signer appears under different keys depending on whether they used a session key or signed directly.

### Affected locations

**Async paths (need `resolve_canonical_key` calls):**
- `transaction_create.rs:225` — `signer: Some(creator.key())`
- `settings_transaction_create.rs:156` — `signer: Some(creator.key())`
- `transaction_execute.rs:255,267` — `signer: Some(ctx.accounts.signer.key())`
- `settings_transaction_execute.rs:206,222` — `signer: Some(ctx.accounts.signer.key())`
- `use_spending_limit.rs:278` — `signer: ctx.accounts.signer.key()` (native-only, lower priority)

**Sync paths (dump raw remaining_accounts keys):**
- `transaction_execute_sync.rs:254-257` — Settings path signers event
- `transaction_execute_sync.rs:305-308` — Policy path signers event
- `settings_transaction_sync.rs:158-161` — signers event
- `transaction_execute_sync_legacy.rs:150-153` — signers event

**Already correct (for reference):**
- `proposal_create.rs:129` — uses `resolve_canonical_key`
- `proposal_vote.rs:126,172,225` — uses `resolve_canonical_key`

### Fix

**Async paths:** Call `resolve_canonical_key(creator.key(), creator.is_signer)?` before building the event, use the result in the `signer` field. `transaction_create` and `settings_transaction_create` should capture `canonical_key` from `verify_signer`'s return in `validate` and pass it through to the inner function (or re-resolve it).

**Sync paths:** The `signers` field in sync events currently maps raw account keys from `remaining_accounts[..num_signers]`. To fix, resolve each signer's canonical key via `resolve_canonical_key`. Note that `validate_synchronous_consensus` already computes canonical keys internally but doesn't return them. Options:
- Have `validate_synchronous_consensus` return the list of `verified_keys` (already computed at line 95).
- Or resolve separately in the event-building code.

Returning `verified_keys` from `validate_synchronous_consensus` is cleaner and avoids duplicate work.

---

## Issue 5: Session keys on external signers in policies [CONFIRM WITH AUDITORS]

### Problem
`classify_signer` in the "fast path" (`!is_native_tx_signer`) only checks direct external signer matches — it does not check session keys. Session keys are only checked in the "slow path" (`is_native_tx_signer=true`).

This means a session key on an external signer in a policy would fail with `NotASigner` when used via the async path (where the external signer account has `is_signer=false`).

### Question
Is this intentional? Session keys are designed for native tx signing (the session key itself signs the Solana transaction). An external signer doesn't sign the Solana transaction — it provides a cryptographic signature in `extra_verification_data`. So session keys on external signers may not make architectural sense.

If intentional, no fix needed. If not, `classify_signer`'s fast path would need to also check session keys for external signers.

---

## Issue 6: `transaction_buffer_close` has no V2/external signer support [HIGH]

### Problem
`transaction_buffer_close` uses `creator: Signer<'info>` (line 35), meaning only native Solana transaction signers can close buffers. There is no `close_transaction_buffer_v2` variant.

This means: **an external signer can CREATE and EXTEND a buffer (both have V2 variants with `AccountInfo<'info>` + `verify_signer`) but can NEVER close it.** The rent is permanently trapped even if the external signer is still active.

This is a separate issue from canonical keys — even with canonical key fixes, external signers cannot close buffers because the instruction requires a native Solana signature.

### Affected files
- `transaction_buffer_close.rs:35` — `pub creator: Signer<'info>`
- `transaction_buffer_close.rs` — no V2 variant exists
- `mod.rs` — no `close_transaction_buffer_v2` instruction registered

### Fix
Add a `CloseTransactionBufferV2` account struct and `close_transaction_buffer_v2` instruction handler that:
1. Changes `creator` from `Signer<'info>` to `AccountInfo<'info>`
2. Adds `consensus_account` for `verify_signer` call
3. Accepts `extra_verification_data: Option<ExtraVerificationData>`
4. Verifies the signer via `verify_signer` (supports native, session key, and external)
5. Checks canonical key matches stored `transaction_buffer.creator`

This should be implemented together with Issue 1 (canonical key for buffer creator), since both modify the same instruction and the close V2 handler should use canonical key matching from the start.

Note: the `close = creator` Anchor directive sends rent to `creator`. In V2, if the canonical signer is an external signer (not a native account), rent should go to `rent_payer` or a separate `rent_collector` account instead. This needs a design decision — either:
- Add a `rent_collector` field to `TransactionBuffer` (stored at creation, like `Transaction` and `Batch` already do)
- Or send rent to the `rent_payer` account passed in the close instruction

`TransactionBuffer` currently does NOT have a `rent_collector` field (unlike `Transaction`, `SettingsTransaction`, and `Batch` which all do). Adding one aligns it with the rest of the codebase.

---

## Issue 7: `transaction_buffer_close` doesn't verify signer membership [MEDIUM]

### Problem
Even in the current V1 path, `transaction_buffer_close` only checks:
1. `creator: Signer<'info>` — the account signed the transaction
2. `transaction_buffer.creator == creator.key()` — matches the stored creator
3. PDA seed derivation

It does **not** verify that the creator is still a signer on the consensus account. There is no `verify_signer` call and no membership check. This means a revoked signer can still close buffers they created (and reclaim rent).

### Question
Is this intentional? The comment says "Account can be closed anytime by the creator, regardless of the current settings transaction index." This suggests it's by design — the creator should always be able to clean up their own buffers. But a revoked signer reclaiming rent could be considered a concern.

If intentional, document it explicitly. If not, the V2 fix (Issue 6) should include a membership check via `verify_signer`.

---

## Issue 8: `use_spending_limit` is native-only, no V2 [LOW / BY DESIGN?]

### Problem
`use_spending_limit` uses `signer: Signer<'info>` and checks `spending_limit.signers.contains(&signer.key())`. There is no V2 variant. External signers cannot use spending limits.

### Context
The legacy spending limit system (on `Settings`) is separate from policy-based spending limits. Policy-based spending limits go through the policy consensus path which already supports V2. The question is whether the legacy `use_spending_limit` instruction needs external signer support.

### Question
Is this intentional? If legacy spending limits are being deprecated in favor of policy-based ones, no fix needed. If not, a V2 variant would be needed with `verify_signer` and canonical key resolution for the `spending_limit.signers` check.

---

## Issue 9: `transaction_create` and `settings_transaction_create` store raw key as creator [LOW]

### Problem
`transaction_create.rs:167` stores `creator.key()` and `settings_transaction_create.rs:130` stores `creator.key()`. While these `creator` fields are not used for authorization checks downstream (only for event logging and indexing), they permanently record the wrong identity when a session key creates the transaction.

The `Transaction` and `SettingsTransaction` state is visible to off-chain indexers. If a session key creates a transaction, the stored `creator` field will be the session key pubkey rather than the canonical parent signer key.

### Affected files
- `transaction_create.rs:167` — `transaction.creator = creator.key()`
- `settings_transaction_create.rs:130` — `transaction.creator = creator.key()`

### Fix
Capture canonical key from `verify_signer` return value (already called in `validate`) and store that instead. Same pattern as `proposal_create.rs:129` which already does `resolve_canonical_key`.

```rust
// transaction_create.rs — in create_transaction_inner:
let canonical_key = consensus_account.resolve_canonical_key(creator.key(), creator.is_signer)?;
transaction.creator = canonical_key;
```

---

## Summary Table

| # | Issue | Severity | Complexity | PDA Change | SDK Change |
|---|---|---|---|---|---|
| 1 | TransactionBuffer creator raw key | **HIGH** | High | Yes | Yes |
| 2 | Batch creator raw key | **HIGH** | Low | No | Yes |
| 6 | Buffer close missing V2 | **HIGH** | Medium | No* | Yes |
| 3 | increment_account_index_v2 no session keys | **MEDIUM** | Medium | No | No |
| 7 | Buffer close no membership check | **MEDIUM** | Low | No | No |
| 9 | Transaction/SettingsTransaction creator raw key | **LOW** | Low | No | No |
| 4 | Event logging raw keys (~10 locations) | **LOW** | Low | No | No |
| 8 | use_spending_limit native-only | **LOW/BY DESIGN** | Medium | No | Yes |
| 5 | Session keys on external signers in policies | **TBD** | Low | No | No |

*Issue 6 should be implemented together with Issue 1 since both modify buffer close.

## Implementation Order

1. **Issues 1 + 6 (TransactionBuffer)** — fix together: canonical key in PDA seeds + add V2 close + add `rent_collector` field to `TransactionBuffer`
2. **Issue 2 (Batch)** — simple canonical key fix, no PDA changes
3. **Issue 9 (Transaction/SettingsTransaction creator)** — simple, store canonical key
4. **Issue 3 (increment_account_index_v2)** — refactor to use `verify_signer`
5. **Issue 4 (Events)** — widespread but mechanical
6. **Issues 5, 7, 8** — need design decisions, discuss with auditors

## SDK Impact

- Issues 1+6: PDA derivation for transaction buffers changes to use canonical key; new `close_transaction_buffer_v2` instruction; `TransactionBuffer` gets `rent_collector` field (deserialization change)
- Issue 2: SDK `batch_add_transaction` must pass canonical signer key for creator check
- Issue 9: `Transaction.creator` and `SettingsTransaction.creator` will now store canonical key (indexing change)
- Issues 3-5, 7: program-only, no SDK impact
- Issue 8: if fixed, new `use_spending_limit_v2` instruction in SDK
