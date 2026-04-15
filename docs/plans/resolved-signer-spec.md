# Spec: `ResolvedSigner` Account Type + Rename `canonical_key` → `resolved_key`

## Problem

Every V2 instruction that stores or emits a signer key manually calls `resolve_canonical_key()` or captures verify_signer's return. This is scattered across ~15 call sites with:
- `/// CHECK: Verified via verify_signer` boilerplate on raw `AccountInfo`
- Manual `resolve_canonical_key(creator.key(), creator.is_signer)?` in handler bodies
- Double classification when validate calls `verify_signer` then handler calls `resolve_canonical_key`
- Inconsistent naming (`canonical_key`, `canonical_signer`, various local bindings)

## Solution

### 1. New type: `ResolvedSigner<'info>`

A thin Anchor-compatible wrapper around `AccountInfo` with lazy resolution via `OnceCell`:

```rust
use std::cell::OnceCell;

pub struct ResolvedSigner<'info> {
    info: AccountInfo<'info>,
    resolved: OnceCell<Pubkey>,
}

impl<'info> ResolvedSigner<'info> {
    /// Resolve the signer's key against a consensus account.
    /// First call computes + stores. Subsequent calls return cached &Pubkey.
    /// OnceCell provides interior mutability via &self — no mut required.
    pub fn resolve(&self, consensus: &impl Consensus) -> Result<&Pubkey> {
        self.resolved.get_or_try_init(|| {
            consensus.resolve_signer_key(*self.info.key, self.info.is_signer)
        })
    }

    pub fn key(&self) -> Pubkey {
        *self.info.key
    }

    pub fn is_signer(&self) -> bool {
        self.info.is_signer
    }

    pub fn to_account_info(&self) -> AccountInfo<'info> {
        self.info.clone()
    }
}
```

Must implement Anchor traits to work in `#[derive(Accounts)]`:
- `ToAccountInfo<'info>`
- `ToAccountMetas`
- `Key`
- The `Accounts` trait (similar to how `UncheckedAccount` / `Signer` work — wrap the AccountInfo during `try_accounts`)

### 2. Usage in account context

The resolved key is available directly in Anchor seed expressions because `resolve()` returns `&Pubkey` borrowing from the struct's `OnceCell`. Fields are processed top-to-bottom, so resolution happens before downstream seeds evaluate:

```rust
#[derive(Accounts)]
#[instruction(args: CreateTransactionBufferArgs)]
pub struct CreateTransactionBuffer<'info> {
    #[account(mut)]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    /// Signer resolved against consensus_account.
    pub creator: ResolvedSigner<'info>,

    #[account(
        init,
        payer = rent_payer,
        space = TransactionBuffer::size(args.final_buffer_size)?,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION_BUFFER,
            creator.resolve(&consensus_account)?.as_ref(),
            &args.buffer_index.to_le_bytes(),
        ],
        bump
    )]
    pub transaction_buffer: Account<'info, TransactionBuffer>,

    #[account(mut)]
    pub rent_payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
```

In handlers — cache hit, no re-scan:
```rust
fn create_transaction_inner(ctx: Context<Self>, args: ...) -> Result<()> {
    let resolved_key = ctx.accounts.creator.resolve(&ctx.accounts.consensus_account)?;
    transaction.creator = *resolved_key;
    // ...
    signer: Some(*resolved_key),
}
```

### 3. Rename: `canonical_key` → `resolved_key`

Global rename across the codebase:

| Old | New |
|-----|-----|
| `resolve_canonical_key` (method on Consensus trait) | `resolve_signer_key` |
| `canonical_key` (local variable) | `resolved_key` |
| `canonical_signer` (local variable) | `resolved_signer` |
| `ClassifiedSigner::signer().key()` → used for "canonical" | Same, but callers rename their bindings |

### 4. Instructions to update

All instructions with `pub creator: AccountInfo<'info>` or `pub signer: AccountInfo<'info>` that call `resolve_canonical_key` or capture `verify_signer`'s return:

| Instruction | Field | Currently |
|---|---|---|
| `batch_add_transaction` | `signer: AccountInfo` | captures verify_signer return as `canonical_key` |
| `batch_create` | `creator: AccountInfo` | calls `resolve_canonical_key` in inner |
| `increment_account_index` (V2) | `signer: AccountInfo` | captures verify_signer return as `canonical_key` |
| `settings_transaction_create` | `creator: AccountInfo` | calls `resolve_canonical_key` in inner |
| `settings_transaction_execute` | `signer: AccountInfo` | calls `resolve_canonical_key` in inner |
| `settings_transaction_sync` | signers via remaining_accounts | calls `resolve_canonical_key` in event map |
| `transaction_buffer_close` | `creator: AccountInfo` | calls `resolve_canonical_key` in V2 handler |
| `transaction_buffer_create` | `creator: AccountInfo` | calls `resolve_canonical_key` in V2 handler |
| `transaction_buffer_extend` | `creator: AccountInfo` | calls `resolve_canonical_key` in V2 handler |
| `transaction_create` | `creator: AccountInfo` | calls `resolve_canonical_key` in inner |
| `transaction_create_from_buffer` | `creator: AccountInfo` | calls `resolve_canonical_key` in V2 handler |
| `transaction_execute` | `signer: AccountInfo` | calls `resolve_canonical_key` in inner |
| `transaction_execute_sync` | signers via remaining_accounts | calls `resolve_canonical_key` in event map |
| `transaction_execute_sync_legacy` | signers via remaining_accounts | calls `resolve_canonical_key` in event map |
| `proposal_create` | `creator: AccountInfo` | captures verify_signer + calls `resolve_canonical_key` |
| `proposal_vote` | `signer: AccountInfo` | calls `resolve_canonical_key` in handlers |

Each `AccountInfo` field becomes `ResolvedSigner`. Each `resolve_canonical_key(key, is_signer)?` call becomes `field.resolve(&consensus)?`.

**Note:** `smart_account_create` (`creator: Signer`), `use_spending_limit` (`signer: Signer`), and `increment_account_index` V1 (`signer: Signer`) stay as `Signer` — they don't need resolution.

**Note:** Sync instructions (settings_transaction_sync, transaction_execute_sync, transaction_execute_sync_legacy) resolve signers from `remaining_accounts`, not a named field. These keep calling `resolve_signer_key` directly on the consensus account for the remaining_accounts map.

### 5. Consensus trait change

```rust
// Before
fn resolve_canonical_key(&self, signer_key: Pubkey, is_native_tx_signer: bool) -> Result<Pubkey>

// After
fn resolve_signer_key(&self, signer_key: Pubkey, is_native_tx_signer: bool) -> Result<Pubkey>
```

`verify_signer` return type stays `Result<Pubkey>` — callers rename their local binding from `canonical_key` to `resolved_key`.

### 6. File location

`ResolvedSigner` lives in a new module:
```
programs/squads_smart_account_program/src/state/resolved_signer.rs
```

Re-exported from `state/mod.rs` so instructions can use `use crate::state::ResolvedSigner`.

### 7. What does NOT change

- `verify_signer` stays in validate via `#[access_control]` — it handles crypto verification, nonce/counter updates, permission checks
- `ResolvedSigner` is purely key resolution, not authentication
- The close buffer fast path (direct key match for removed native signers) stays
- V1 instructions that use `Signer<'info>` stay as-is

### 8. Risk: Anchor proc macro + `?` in seeds

The `creator.resolve(&consensus_account)?.as_ref()` expression in seeds relies on Anchor passing `?` through to generated code. This needs to be validated by building the program. If Anchor's macro chokes on `?` in seeds, the fallback is:
- Use a constraint to resolve: `#[account(constraint = creator.resolve(&consensus_account).is_ok())]`
- Use `creator.resolve(&consensus_account).unwrap().as_ref()` in seeds (safe because constraint ran first)
- Or keep manual PDA derivation for init cases only

### 9. Validation

- `anchor build -- --features=testing` must succeed
- All existing tests must pass (3 expected failures only)
- No new `/// CHECK` annotations needed for signer fields
- No `resolve_canonical_key` calls remain outside of `ResolvedSigner::resolve()` and sync remaining_accounts maps
