---
title: "fix: Canonical Key Audit Remediation"
type: fix
date: 2026-03-30
pr: 30
severity: high
---

# Canonical Key Audit Remediation

## Problem

`verify_signer()` returns the canonical key (parent signer's key for session keys), but most handlers discard it — storing/comparing raw `creator.key()` instead. This causes:

1. **Locked state:** session key creates buffer/batch → session key expires → buffer/batch permanently locked
2. **Missing V2 close:** `transaction_buffer_close` requires `Signer<'info>` — external signers can never close buffers
3. **Broken V2 path:** `create_from_buffer_v2` double-calls `verify_signer`, breaking external signers (nonce increment)
4. **Wrong events:** ~10 locations log raw key instead of canonical key

## Pre-Implementation Gates

Confirm with auditors:

- `classify_signer` fast path excludes session keys for external signers — believed intentional (session keys always `is_signer=true`)
- V1 buffer close allows revoked signers, V2 requires membership — confirm asymmetry is acceptable
- `use_spending_limit` native-only — out of scope (superseded by policy-based spending limits)

---

## Design Decisions

### D1: Canonical key in PDA — enforced in validate(), not in struct

PDA seeds use `creator.key()` (same expression as V1). Canonical key enforcement is in `validate()`:

```rust
let canonical_key = consensus_account.verify_signer(creator, ...)?;
require!(creator.key() == canonical_key, SmartAccountError::InvalidCanonicalCreator);
```

SDK passes canonical key as `creator` for V2. Session key passed instead → `verify_signer` returns parent → check fails → rejected.

- **Native:** canonical = raw → passes
- **External:** canonical = key_id → passes (verified via EVD)
- **Session key users:** use V1 for native signing, or pass parent key + EVD for V2

V2-created buffers: PDA seeded with canonical key → any signer resolving to same canonical key can extend/close.
V1-created buffers: PDA seeded with raw key → only original signer can operate (unchanged).

### D2: No new account types, no new args

Reuse `TransactionBuffer` and existing args structs unchanged. No Borsh changes, no migration.

### D3: Buffer close V2 uses `close = creator`

No `rent_collector` field. Rent goes to `creator` (the canonical key account). Any Solana account can receive lamports.

### D4: `create_from_buffer` double verify — bug fix

Current V2 path is broken for external signers (double nonce). Fix: call `create_transaction_inner` directly (change to `pub(crate)`). Add missing `is_active()` + `validate_account_index_unlocked()` checks to `create_from_buffer.validate()`.

---

## Implementation

### Phase 1: TransactionBuffer [HIGH]

#### 1.1 Add canonical key check to V2 buffer validates

**Files:** `transaction_buffer_create.rs`, `transaction_buffer_extend.rs`, `transaction_create_from_buffer.rs`

Each V2 validate already calls `verify_signer` but discards the return. Capture it and add:

```rust
let canonical_key = consensus_account.verify_signer(creator, ...)?;
require!(creator.key() == canonical_key, SmartAccountError::InvalidCanonicalCreator);
```

For extend and close V2, also check stored creator matches:
```rust
require!(transaction_buffer.creator == canonical_key, SmartAccountError::Unauthorized);
```

V1 validates unchanged.

#### 1.2 Add `close_transaction_buffer_v2` — update existing struct

**File:** `transaction_buffer_close.rs`

Change `creator: Signer<'info>` to `creator: AccountInfo<'info>`. V1 validate adds `require!(creator.is_signer)`. Add V2 validate + entry point:

```rust
fn validate(&self) -> Result<()> {
    require!(self.creator.is_signer, SmartAccountError::MissingSignature);
    Ok(())
}

fn validate_v2(&mut self, remaining_accounts: &[AccountInfo], evd: Option<ExtraVerificationData>) -> Result<()> {
    let message = create_transaction_buffer_close_message(
        &self.transaction_buffer.key(), &self.consensus_account.key(),
    );
    let canonical_key = self.consensus_account.verify_signer(
        &self.creator, remaining_accounts, message, evd.as_ref(), None,
    )?;
    require!(self.creator.key() == canonical_key, SmartAccountError::InvalidCanonicalCreator);
    require!(self.transaction_buffer.creator == canonical_key, SmartAccountError::Unauthorized);
    Ok(())
}

#[access_control(ctx.accounts.validate())]
pub fn close_transaction_buffer(ctx: Context<Self>) -> Result<()> { Ok(()) }

#[access_control(ctx.accounts.validate_v2(&ctx.remaining_accounts, extra_verification_data))]
pub fn close_transaction_buffer_v2(ctx: Context<Self>, extra_verification_data: Option<ExtraVerificationData>) -> Result<()> { Ok(()) }
```

#### 1.3 Fix `create_from_buffer` double verify (D4)

**File:** `transaction_create_from_buffer.rs`

Call `CreateTransaction::create_transaction_inner` directly instead of `create_transaction` (which re-runs validate).

**File:** `transaction_create.rs` — change `fn create_transaction_inner` to `pub(crate) fn`.

Add missing checks to `create_from_buffer` validate (both V1 and V2):
```rust
consensus_account.is_active(remaining_accounts)?;
if consensus_account.account_type() == ConsensusAccountType::Settings {
    let settings = consensus_account.read_only_settings()?;
    settings.validate_account_index_unlocked(account_index)?;
}
```

#### 1.4 Add error variant + message function

**File:** `errors.rs` — add `InvalidCanonicalCreator`

**File:** `messages.rs` — add:
```rust
pub fn create_transaction_buffer_close_message(buffer_key: &Pubkey, consensus_account_key: &Pubkey) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"tx_buffer_close_v2");
    hasher.hash(buffer_key.as_ref());
    hasher.hash(consensus_account_key.as_ref());
    hasher
}
```

#### 1.5 Register + SDK

**File:** `lib.rs` — register `close_transaction_buffer_v2` in V2 section.

**SDK:** Add `closeTransactionBufferV2` across instructions/transactions/rpc layers with `isSigner` override. V2 buffer wrappers pass canonical key as `creator`.

#### 1.6 Tests

- External signer V2 create → V2 close (canonical key in PDA)
- New session key of same parent can extend + close V2-created buffer
- Session key passed as creator to V2 create → fails (canonical check)
- Native signer V2 create + V2 close works
- V1 create + V1 close still works (backward compat)
- `create_from_buffer_v2` works for external signers (D4 fix)

---

### Phase 2: Batch [HIGH]

#### 2.1 `batch_create`

**File:** `batch_create.rs` — in `create_batch_inner`:
```rust
let canonical_key = settings.resolve_canonical_key(creator.key(), creator.is_signer)?;
batch.creator = canonical_key;
```

#### 2.2 `batch_add_transaction`

**File:** `batch_add_transaction.rs` — in `validate`, replace `signer.key() == batch.creator` with:
```rust
let canonical_key = settings.verify_signer(signer, ...)?;
require!(canonical_key == batch.creator, SmartAccountError::Unauthorized);
```

#### 2.3 Tests

- Session key creates batch → expires → new session key adds transactions
- Different canonical key fails to add transactions

---

### Phase 3: Stored Fields + Events [LOW]

Use `resolve_canonical_key` (lightweight, no crypto) in inner functions. Do NOT change `validate_synchronous_consensus` return type (`#[access_control]` expects `Result<()>`).

#### 3.1 Transaction/SettingsTransaction creator

**Files:** `transaction_create.rs`, `settings_transaction_create.rs`

In `_inner`:
```rust
let canonical_key = consensus_account.resolve_canonical_key(creator.key(), creator.is_signer)?;
transaction.creator = canonical_key;
// Event:
signer: Some(canonical_key),
```

#### 3.2 Execute event logging

**Files:** `transaction_execute.rs`, `settings_transaction_execute.rs`

Replace `signer: Some(ctx.accounts.signer.key())` with `resolve_canonical_key`.

#### 3.3 Sync event logging

**Files:** `transaction_execute_sync.rs`, `settings_transaction_sync.rs`, `transaction_execute_sync_legacy.rs`

```rust
signers: ctx.remaining_accounts[..args.num_signers as usize]
    .iter()
    .map(|acc| consensus_account.resolve_canonical_key(*acc.key, acc.is_signer)
        .unwrap_or(*acc.key))
    .collect(),
```

---

### Phase 4: increment_account_index_v2 [MEDIUM]

**File:** `increment_account_index.rs`

Replace ~50-line manual verification in V2 validate with `verify_signer` + manual OR permission check:

```rust
let canonical_key = settings.verify_signer(&self.signer, remaining_accounts, message, evd.as_ref(), None)?;

let signer = settings.is_signer_v2(canonical_key).ok_or(SmartAccountError::NotASigner)?;
require!(
    signer.permissions().has(Permission::Initiate)
        || signer.permissions().has(Permission::Vote)
        || signer.permissions().has(Permission::Execute),
    SmartAccountError::Unauthorized
);
```

`create_increment_account_index_message` already exists in `messages.rs`.

---

## Acceptance Criteria

- [ ] V2-created buffer with canonical key PDA → closeable by any signer resolving to same canonical key
- [ ] External signer can close buffer via V2
- [ ] Session key as creator rejected by V2 create (canonical check fails)
- [ ] Batch: session key creates → expires → new session key of same parent can add transactions
- [ ] `Transaction.creator` / `SettingsTransaction.creator` store canonical key
- [ ] Events log canonical keys
- [ ] V1 instructions fully backward compatible
- [ ] Session key can call `increment_account_index_v2`
- [ ] `create_from_buffer_v2` works for external signers

## Files Affected

| File | Phase | Change |
|---|---|---|
| `transaction_buffer_create.rs` | 1 | Add canonical key check to V2 validate |
| `transaction_buffer_extend.rs` | 1 | Add canonical key check to V2 validate |
| `transaction_buffer_close.rs` | 1 | `Signer` → `AccountInfo`, add V2 validate + entry point |
| `transaction_create_from_buffer.rs` | 1 | Canonical key check, `is_active`/`validate_account_index_unlocked`, call `_inner` directly |
| `transaction_create.rs` | 1,3 | `pub(crate) fn create_transaction_inner`; store canonical key + fix event |
| `errors.rs` | 1 | Add `InvalidCanonicalCreator` |
| `messages.rs` | 1 | Add `create_transaction_buffer_close_message` |
| `lib.rs` | 1 | Register `close_transaction_buffer_v2` |
| `batch_create.rs` | 2 | Store canonical key |
| `batch_add_transaction.rs` | 2 | Compare canonical keys |
| `settings_transaction_create.rs` | 3 | Store canonical key, fix event |
| `transaction_execute.rs` | 3 | Canonical key in events |
| `settings_transaction_execute.rs` | 3 | Canonical key in events |
| `transaction_execute_sync.rs` | 3 | Canonical key in sync events |
| `settings_transaction_sync.rs` | 3 | Canonical key in sync events |
| `transaction_execute_sync_legacy.rs` | 3 | Canonical key in sync events |
| `increment_account_index.rs` | 4 | Replace manual verify with `verify_signer` |

**SDK:** `closeTransactionBufferV2` (new, 3 layers) + update V2 buffer wrappers to pass canonical key as `creator`.
