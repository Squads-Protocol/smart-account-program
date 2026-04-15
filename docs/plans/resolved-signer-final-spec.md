# ResolvedSigner — Final Spec

## What we learned

We tried three approaches:
1. **`#[derive(Accounts)]` composite** — Anchor generates CPI/client modules automatically. No `OnceCell` (can't have non-account fields). Compiled but no caching.
2. **`InterfaceAccount<ResolvedSigner>`** — `OnceCell` works, CPI modules work. But `InterfaceAccount::try_from` has a hardcoded `AccountNotInitialized` check that rejects system-owned accounts with 0 lamports. External signers fail at runtime.
3. **Manual `Accounts` impl** — `OnceCell` works, no owner/lamport checks. But Anchor's derive macro expects `__client_accounts_resolved_signer` and `__cpi_client_accounts_resolved_signer` modules as siblings. These must be actual `mod` declarations reachable via `use crate::instructions::*` (not `pub use` re-exports — Rust doesn't propagate modules through glob re-exports of re-exports, only through direct child module declarations).

**Winner: approach 3** with the CPI/client modules defined in `instructions/mod.rs` as direct `mod` declarations.

## Architecture

### ResolvedSigner (state/resolved_signer.rs)

```rust
pub struct ResolvedSigner<'info> {
    info: AccountInfo<'info>,
    classified: OnceCell<ClassifiedSigner>,
    resolved_key: OnceCell<Pubkey>,
}
```

Three methods:
- `resolve(&self, consensus: &T) -> Result<&Pubkey>` — classify + cache, returns `&Pubkey` from OnceCell
- `resolved_key(&self) -> Result<&Pubkey>` — read cached key
- `classified(&self) -> Result<&ClassifiedSigner>` — read cached classification

Manual trait impls: `Accounts`, `ToAccountMetas`, `ToAccountInfos`, `AccountsExit`, `Key`, `AsRef<AccountInfo>`.

`ResolvedSignerBumps` empty struct + `Bumps` impl (required by Anchor for composite fields).

No `CheckOwner`, no `AccountDeserialize`, no `AccountSerialize`. Just pops an `AccountInfo` from the accounts slice.

### Anchor boilerplate modules (instructions/mod.rs)

Two `mod` declarations that must live in `instructions/mod.rs` so the derive macro finds them as siblings:

```rust
pub(crate) mod __client_accounts_resolved_signer {
    // Pubkey-based struct for Rust client instruction building
    // Must impl BorshSerialize + ToAccountMetas
}

pub(crate) mod __cpi_client_accounts_resolved_signer {
    // AccountInfo-based struct for CPI calls
    // Must impl ToAccountMetas + ToAccountInfos
}
```

**Why here**: Anchor's derive macro generates `use super::*` inside `__client_accounts_X` modules. `super` is the instruction file. Instruction files that have `use crate::instructions::*` bring in items from `instructions/mod.rs` — including these module declarations. The derive's `pub use __client_accounts_resolved_signer::ResolvedSigner` then resolves.

### ClassifiedSigner changes (interface/consensus_trait.rs)

Add `Clone` derive to `ClassifiedSigner` (needed by `OnceCell::set`).

Add `resolved_key()` method:
```rust
impl ClassifiedSigner {
    pub fn resolved_key(&self) -> Pubkey {
        self.signer().key()
    }
}
```

Extract `verify_classified_signer` from `verify_signer`:
```rust
fn verify_classified_signer(
    &mut self,
    classified: &ClassifiedSigner,
    remaining_accounts: &[AccountInfo],
    non_hashed_message: Hasher,
    extra_verification_data: Option<&ExtraVerificationData>,
    required_permission: Option<Permission>,
) -> Result<Pubkey>
```

`verify_signer` becomes: `classify_signer` → `verify_classified_signer`. No behavior change.

`verify_classified_signer` gets `signer_key` for nonce updates from `classified.signer().key()` and skips the `is_signer` check (guaranteed by `classify_signer` — Native/SessionKey only returned when `is_native_tx_signer` was true).

### Rename

Global rename across all `.rs` files:
- `resolve_canonical_key` → `resolve_signer_key` (method on Consensus trait)
- `canonical_key` → `resolved_key` (local variables)
- `canonical_signer` → `resolved_signer` (local variables)
- `canonical key` → `resolved key` (comments)

## Instructions to convert

### Convert to `ResolvedSigner<'info>` (16 instructions)

Each `pub creator: AccountInfo<'info>` or `pub signer: AccountInfo<'info>` becomes `pub creator: ResolvedSigner<'info>` or `pub signer: ResolvedSigner<'info>`.

**NO `#[account(constraint = ...)]`** on the ResolvedSigner field — Anchor doesn't support constraints on composite fields.

Files with `use crate::state::*` already import `ResolvedSigner`. Files with structured imports need to add `ResolvedSigner` to their import list.

Every file using `ResolvedSigner` needs `use crate::instructions::*;` if not already present (for the CPI/client modules).

| Instruction | Field | Consensus account | validate changes | handler changes |
|---|---|---|---|---|
| `transaction_buffer_create` | `creator` | `consensus_account` | `verify_classified_signer(creator.classified()?, ...)` | `*creator.resolved_key()?` in seeds + handler |
| `transaction_create` | `creator` | `consensus_account` | same | `*creator.resolved_key()?` for `transaction.creator` + event |
| `settings_transaction_create` | `creator` | `settings` | same | `*creator.resolved_key()?` for `transaction.creator` + event |
| `batch_create` | `creator` | `settings` | same | `*creator.resolved_key()?` for `batch.creator` |
| `proposal_create` | `creator` | `consensus_account` | `verify_classified_signer(..., None)` + manual Initiate OR Vote check using `*creator.resolved_key()?` | `*creator.resolved_key()?` for event |
| `transaction_buffer_extend` | `creator` | `consensus_account` | same | `*creator.resolved_key()?` for V1/V2 creator check |
| `transaction_buffer_close` | `creator` | `consensus_account` | fast path uses `creator.to_account_info().is_signer` + `creator.key()`; slow path calls `creator.resolve(...)` then `verify_classified_signer` | `*creator.resolved_key()?` for V1/V2 check |
| `transaction_create_from_buffer` | `creator` | `transaction_create.consensus_account` | same | `*creator.resolved_key()?` for V1/V2 check |
| `transaction_execute` | `signer` | `consensus_account` | same | `*signer.resolved_key()?` for events |
| `settings_transaction_execute` | `signer` | `settings` | same | `*signer.resolved_key()?` for events |
| `batch_add_transaction` | `signer` | `settings` | `verify_classified_signer` + `*signer.resolved_key()?` for batch.creator check | — |
| `batch_execute_transaction` | `signer` | `settings` | same | — |
| `proposal_vote` | `signer` | `consensus_account` | same | `*signer.resolved_key()?` in approve/reject/cancel (x3) |
| `activate_proposal` | `signer` | `settings` | same | — |
| `create_session_key` | `signer` | `settings` | custom verification (not verify_signer) — just change field type, use `signer.key()` for lookups | — |
| `increment_account_index` (V2) | `signer` | `settings` | `verify_classified_signer` + `*signer.resolved_key()?` for is_signer_v2 + OR perm check | — |

### DO NOT convert (3 instructions)

| Instruction | Field | Reason |
|---|---|---|
| `smart_account_create` | `creator: Signer` | No consensus account |
| `use_spending_limit` | `signer: Signer` | Not consensus-based |
| `increment_account_index` V1 | `signer: Signer` | V1 only, native signers only |

### Rename only, no type conversion (3 sync instructions)

| Instruction | Change |
|---|---|
| `settings_transaction_sync` | `resolve_canonical_key` → `resolve_signer_key` |
| `transaction_execute_sync` | same |
| `transaction_execute_sync_legacy` | same |

## Per-instruction conversion pattern

### Step 1: Account struct

```rust
// Before:
/// CHECK: Verified via verify_signer
pub creator: AccountInfo<'info>,

// After (no #[account] attribute — composite fields don't support constraints):
pub creator: ResolvedSigner<'info>,
```

### Step 2: Imports

Each instruction file needs:
- `ResolvedSigner` in scope (via `use crate::state::*` or explicit import)
- `use crate::instructions::*;` (for CPI/client modules — add if missing)

### Step 3: validate — resolve + verify_classified_signer

```rust
// Before:
consensus_account.verify_signer(
    &creator,
    remaining_accounts,
    message,
    extra_verification_data.as_ref(),
    Some(Permission::Initiate),
)?;

// After:
creator.resolve(&consensus_account)?;
consensus_account.verify_classified_signer(
    creator.classified()?,
    remaining_accounts,
    message,
    extra_verification_data.as_ref(),
    Some(Permission::Initiate),
)?;
```

For `transaction_buffer_create`: `resolve()` already runs in seeds — just call `verify_classified_signer` in validate.

For all others: call `resolve()` at the top of validate, then `verify_classified_signer`.

### Step 4: handler — resolved_key()

```rust
// Before:
let canonical_key = consensus_account.resolve_canonical_key(creator.key(), creator.is_signer)?;
transaction.creator = canonical_key;

// After:
transaction.creator = *creator.resolved_key()?;
```

### Step 5: AccountInfo access

Where code needs `&AccountInfo` (e.g. for message building `creator.key()`):
- `creator.key()` works directly (impl Key)
- `creator.to_account_info()` for full AccountInfo
- `creator.as_ref()` for `&AccountInfo`
- `*creator.info.key` for the raw key (info is a field on the struct)

### Special cases

**transaction_buffer_create** — `resolve()` in seeds:
```rust
seeds = [
    SEED_PREFIX,
    consensus_account.key().as_ref(),
    SEED_TRANSACTION_BUFFER,
    creator.resolve(&consensus_account)?.as_ref(),
    &args.buffer_index.to_le_bytes(),
],
```

**transaction_buffer_close** — fast path for removed members:
```rust
let creator_info = self.creator.to_account_info();
if creator_info.is_signer && self.creator.key() == self.transaction_buffer.creator {
    return Ok(());
}
self.creator.resolve(&self.consensus_account)?;
self.consensus_account.verify_classified_signer(
    self.creator.classified()?, ...
)?;
```

**create_session_key** — has its own verification (not verify_signer):
Just change the field type. The custom precompile/syscall verification in validate uses `self.signer.key()` and `*self.signer.info.key` — update to use the new struct's API.

## Execution order

1. Add `resolved_signer.rs` to `state/mod.rs`
2. Add CPI/client modules + `Clone` derive to `consensus_trait.rs`
3. Add `verify_classified_signer` to Consensus trait
4. Add `resolved_key()` to `ClassifiedSigner`
5. Rename `resolve_canonical_key` → `resolve_signer_key` on the trait
6. Convert `transaction_buffer_create` first (has seeds — proves the concept)
7. Convert remaining 15 instructions one at a time
8. Global rename `canonical_key` → `resolved_key` in variables and comments
9. Build: `anchor build -- --features=testing`
10. Run standalone test
11. Run full test suite

## Rules

- **NEVER use sed/awk** — all edits via Edit tool, one file at a time
- **NO constraints on ResolvedSigner fields** — Anchor panics on composite field constraints
- **Every instruction file needs `use crate::instructions::*`** if not already present
- **CPI/client modules go in `instructions/mod.rs` only** — not in `resolved_signer.rs`, not in `lib.rs`
- **Test after each batch** — build after converting every 3-4 files
