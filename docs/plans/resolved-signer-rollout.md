# Spec: Roll out `ResolvedSigner` to all instructions

## Summary

Replace every `AccountInfo<'info>` signer/creator field with `InterfaceAccount<'info, ResolvedSigner>` across all **async** (consensus-based) instructions. Sync instructions are out of scope — they resolve signers from `remaining_accounts`, not named fields. Use `verify_classified_signer` (with cached classification from `resolve()`) instead of `verify_signer` (which re-classifies). Rename all `canonical_key`/`canonical_signer` to `resolved_key`/`resolved_signer`.

## Architecture (already implemented + tested)

```
InterfaceAccount<'info, ResolvedSigner>
├── CheckOwner::check_owner() → Ok(())         // no owner check
├── AccountDeserialize::try_deserialize() → OnceCell::new()  // no data read
├── resolve(info, consensus) → &Pubkey          // classify + cache
├── resolved_key() → &Pubkey                    // read cached key
└── classified() → &ClassifiedSigner            // read cached classification
```

Verification stays on the Consensus trait: `consensus.verify_classified_signer(info, creator.classified()?, ...)`

## What to convert

### Group A: `creator: AccountInfo` → `creator: InterfaceAccount<ResolvedSigner>`

| File | Field | validate uses | handler uses |
|------|-------|---------------|--------------|
| `transaction_buffer_create.rs` | `creator` | `verify_signer` | `resolve_signer_key` | **DONE** |
| `transaction_create.rs` | `creator` | `verify_signer` | `resolve_signer_key` |
| `settings_transaction_create.rs` | `creator` | `verify_signer` | `resolve_signer_key` |
| `batch_create.rs` | `creator` | `verify_signer` | `resolve_signer_key` |
| `proposal_create.rs` | `creator` | `verify_signer` + manual perm | `resolve_signer_key` |
| `transaction_buffer_extend.rs` | `creator` | `verify_signer` | `resolve_signer_key` |
| `transaction_buffer_close.rs` | `creator` | `verify_signer` (fast path + slow path) | `resolve_signer_key` |
| `transaction_create_from_buffer.rs` | `creator` | `verify_signer` | `resolve_signer_key` |

### Group B: `signer: AccountInfo` → `signer: InterfaceAccount<ResolvedSigner>`

| File | Field | validate uses | handler uses |
|------|-------|---------------|--------------|
| `transaction_execute.rs` | `signer` | `verify_signer` | `resolve_signer_key` |
| `settings_transaction_execute.rs` | `signer` | `verify_signer` | `resolve_signer_key` |
| `batch_add_transaction.rs` | `signer` | `verify_signer` (captures return) | — |
| `batch_execute_transaction.rs` | `signer` | `verify_signer` | — |
| `proposal_vote.rs` | `signer` | `verify_signer` | `resolve_signer_key` (x3: approve/reject/cancel) |
| `activate_proposal.rs` | `signer` | `verify_signer` | — |
| `create_session_key.rs` | `signer` | `verify_signer` | — |
| `increment_account_index.rs` (V2) | `signer` | `verify_signer` (captures return) | — |

### Group C: DO NOT convert (keep as-is)

| File | Field | Reason |
|------|-------|--------|
| `smart_account_create.rs` | `creator: Signer` | No consensus account, no resolution needed |
| `use_spending_limit.rs` | `signer: Signer` | Spending limit signer, not consensus-based |
| `increment_account_index.rs` (V1) | `signer: Signer` | V1 only, native signers only |

### Out of scope: Sync instructions

Sync instructions resolve signers from `remaining_accounts`, not named fields. `ResolvedSigner` doesn't apply. They keep calling `resolve_signer_key` directly on the consensus account in event logging.

| File | Status |
|------|--------|
| `settings_transaction_sync.rs` | No conversion. Rename `canonical_key` → `resolved_key` in variables only. |
| `transaction_execute_sync.rs` | No conversion. Rename only. |
| `transaction_execute_sync_legacy.rs` | No conversion. Rename only. |

## Per-instruction conversion pattern

### Step 1: Account struct — change field type

```rust
// Before:
/// CHECK: Verified via verify_signer
pub creator: AccountInfo<'info>,

// After:
pub creator: InterfaceAccount<'info, ResolvedSigner>,
```

### Step 2: Trigger resolve — add constraint OR use in seeds

For instructions that use the resolved key in seeds (e.g. transaction_buffer_create):
```rust
seeds = [
    ...,
    creator.resolve(&creator.to_account_info(), &consensus_account)?.as_ref(),
    ...,
]
```

For all other instructions, add a constraint to trigger resolve:
```rust
#[account(
    constraint = creator.resolve(&creator.to_account_info(), &consensus_account).is_ok() @ SmartAccountError::NotASigner
)]
pub creator: InterfaceAccount<'info, ResolvedSigner>,
```

### Step 3: Validate — use verify_classified_signer with cached classification

```rust
// Before:
consensus_account.verify_signer(&creator, remaining, msg, evd, Some(Permission::Initiate))?;

// After:
let creator_info = creator.to_account_info();
consensus_account.verify_classified_signer(
    &creator_info,
    creator.classified()?,
    remaining,
    msg,
    evd,
    Some(Permission::Initiate),
)?;
```

### Step 4: Handler — use resolved_key()

```rust
// Before:
let canonical_key = consensus_account.resolve_signer_key(creator.key(), creator.is_signer)?;
transaction.creator = canonical_key;

// After:
transaction.creator = *creator.resolved_key()?;
```

### Special case: batch_add_transaction, increment_account_index

These capture verify_signer's return and use it immediately. Same pattern but with resolved_key:

```rust
// Before:
let canonical_key = settings.verify_signer(&signer, ...)?;
require!(canonical_key == batch.creator, ...);

// After (in validate):
let signer_info = signer.to_account_info();
consensus_account.verify_classified_signer(
    &signer_info, signer.classified()?, remaining, msg, evd, Some(Permission::Initiate),
)?;
let resolved_key = *signer.resolved_key()?;
require!(resolved_key == batch.creator, ...);
```

### Special case: transaction_buffer_close fast path

Keep the fast path for removed native signers:
```rust
if self.creator.to_account_info().is_signer && self.creator.key() == self.transaction_buffer.creator {
    return Ok(());
}
// Fall through to verify_classified_signer for session keys / external signers
```

### Special case: proposal_create manual OR permission check

```rust
let creator_info = creator.to_account_info();
consensus_account.verify_classified_signer(
    &creator_info, creator.classified()?, remaining, msg, evd, None,
)?;
let resolved_key = *creator.resolved_key()?;
require!(
    consensus_account.signer_has_permission(resolved_key, Permission::Initiate)
        || consensus_account.signer_has_permission(resolved_key, Permission::Vote),
    SmartAccountError::Unauthorized
);
```

## Rename: `canonical_key` → `resolved_key`

Global find-and-replace across all `.rs` files:

| Old | New |
|-----|-----|
| `let canonical_key =` | `let resolved_key =` |
| `let canonical_signer =` | `let resolved_signer =` |
| `canonical_key` (in comments) | `resolved_key` |
| `Use canonical_key` (in comments) | `Use resolved_key` |
| `canonical key` (in comments) | `resolved key` |

Already done: `resolve_canonical_key` → `resolve_signer_key` on the Consensus trait.

## Rename: comments referencing old patterns

| Old comment | New comment |
|-------------|-------------|
| `/// CHECK: Verified via verify_signer` | removed (not needed for `InterfaceAccount`) |
| `/// CHECK: Verified via verify_signer. Authorization checked in handler body.` | removed |
| `// Resolve canonical key for storage and events` | `// resolved_key() reads cached value from seed/constraint resolution` |
| `// Cached from seed derivation — no re-scan` | can be removed (it's obvious from the type) |

## Consensus trait: verify_classified_signer naming in comments

Update doc comments in `consensus_trait.rs`:
- `canonical key` → `resolved key` in all doc comments on `verify_signer`, `verify_classified_signer`, `resolve_signer_key`, `classify_signer`

## Files changed summary

| File | Change |
|------|--------|
| `state/resolved_signer.rs` | Already done |
| `state/mod.rs` | Already done |
| `interface/consensus_trait.rs` | `verify_classified_signer` already extracted. Rename comments. |
| `instructions/transaction_buffer_create.rs` | Already done |
| `instructions/transaction_create.rs` | Convert creator + rename |
| `instructions/settings_transaction_create.rs` | Convert creator + rename |
| `instructions/batch_create.rs` | Convert creator + rename |
| `instructions/proposal_create.rs` | Convert creator + rename |
| `instructions/transaction_buffer_extend.rs` | Convert creator + rename |
| `instructions/transaction_buffer_close.rs` | Convert creator + rename |
| `instructions/transaction_create_from_buffer.rs` | Convert creator + rename |
| `instructions/transaction_execute.rs` | Convert signer + rename |
| `instructions/settings_transaction_execute.rs` | Convert signer + rename |
| `instructions/batch_add_transaction.rs` | Convert signer + rename |
| `instructions/batch_execute_transaction.rs` | Convert signer + rename |
| `instructions/proposal_vote.rs` | Convert signer + rename |
| `instructions/activate_proposal.rs` | Convert signer + rename |
| `instructions/create_session_key.rs` | Convert signer + rename |
| `instructions/increment_account_index.rs` (V2) | Convert signer + rename |
| `instructions/settings_transaction_sync.rs` | Rename only |
| `instructions/transaction_execute_sync.rs` | Rename only |
| `instructions/transaction_execute_sync_legacy.rs` | Rename only |
| `utils/context_validation.rs` | Rename `canonical_key` → `resolved_key` |
| `lib.rs` | Remove lifetime annotations from buffer_create |

## Validation

1. `anchor build -- --features=testing` — must compile
2. Run full test suite — max 3 expected failures (increment_account_index)
3. No `/// CHECK` annotations remaining on signer/creator fields (except spending limit, smart_account_create)
4. No `resolve_signer_key` calls remaining in instruction handlers (only in `ResolvedSigner::resolve` and sync remaining_accounts maps)
5. No `canonical_key` or `canonical_signer` variable names remaining anywhere
