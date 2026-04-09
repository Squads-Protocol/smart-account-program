# ResolvedSigner Simplification

Simplify the `ResolvedSigner` type from a 3-step API (resolve/classify/verify) to a 2-step API (verify/read), delete the intermediate `ClassifiedSigner` abstraction, and reduce Anchor boilerplate across instruction files.

## Context

The `feat/resolved-signer` branch introduced `ResolvedSigner` as a wrapper around `AccountInfo` with lazy, cached signer classification and key resolution via `OnceCell`. The design works but exposes more surface area than needed:

- Every instruction handler repeats a 3-line sequence: `resolve()` then `classified()` then `verify_classified_signer()`
- `ClassifiedSigner` is never consumed independently from `verify_classified_signer()` -- it's always a pass-through
- Two `OnceCell` fields (`classified` + `resolved_key`) when only `resolved_key` is read after validation
- 16 instruction files each import two Anchor boilerplate modules with `#[allow(unused_imports)]`

## Design

### ResolvedSigner struct

Drop the `classified` OnceCell. Expose two methods: `verify()` and `resolved_key()`.

```rust
pub struct ResolvedSigner<'info> {
    info: AccountInfo<'info>,
    resolved_key: OnceCell<Pubkey>,
}

impl<'info> ResolvedSigner<'info> {
    /// Classify, verify, and cache the resolved key in one call.
    /// Called in validate().
    pub fn verify(
        &self,
        consensus: &mut impl Consensus,
        remaining_accounts: &[AccountInfo],
        message: Hasher,
        evd: Option<&ExtraVerificationData>,
        permission: Option<Permission>,
    ) -> Result<()> {
        let key = consensus.verify_signer(
            &self.info, remaining_accounts, message, evd, permission,
        )?;
        let _ = self.resolved_key.set(key);
        Ok(())
    }

    /// Read the cached resolved key. Called in handler().
    /// Returns Pubkey (Copy), not &Pubkey.
    pub fn resolved_key(&self) -> Result<Pubkey> {
        self.resolved_key.get().copied()
            .ok_or_else(|| error!(SmartAccountError::NotASigner))
    }

    pub fn to_account_info(&self) -> AccountInfo<'info> {
        self.info.clone()
    }
}
```

The validate/handler separation is preserved:
- **validate()**: `signer.verify(...)` -- all checks happen here, key cached as side effect
- **handler()**: `signer.resolved_key()` -- zero-cost read from OnceCell, pure business logic

Anchor trait impls (`Accounts`, `ToAccountMetas`, `ToAccountInfos`, `AccountsExit`, `Key`, `AsRef`) remain unchanged. The dummy IDL struct, `ResolvedSignerBumps`, and client/CPI modules stay (Anchor requires them).

### Consensus trait changes

Delete from the trait:
- `classify_signer()` -- was only called by `resolve()` and `verify_signer()`
- `verify_classified_signer()` -- was only called with the output of `classify_signer()`
- `ClassifiedSigner` enum -- intermediate type, never consumed independently

Fold classification logic directly into `verify_signer()`:

```rust
fn verify_signer(
    &mut self,
    signer_info: &AccountInfo,
    remaining_accounts: &[AccountInfo],
    non_hashed_message: Hasher,
    extra_verification_data: Option<&ExtraVerificationData>,
    required_permission: Option<Permission>,
) -> Result<Pubkey>
```

`verify_signer` becomes the single public method for async signer verification. It:
1. Scans the signers list to determine type (native/session key/external)
2. For session keys: checks expiration (`session_key_data.expiration > now`) -- this check currently lives in `verify_classified_signer`'s `SessionKey` arm and must be preserved
3. Verifies signature and permissions
4. Applies counter/nonce updates for external signers
5. Returns the resolved key (canonical signer key)

Keep `resolve_signer_key()` as a separate lightweight method for key resolution without verification. The sync path uses it for event logging (e.g., `settings_transaction_sync.rs`, `transaction_execute_sync.rs`). Rewrite its internals to not depend on `ClassifiedSigner` -- just scan and return the canonical key directly.

### Instruction file changes

Before (every instruction, 3 lines + 2 boilerplate imports):
```rust
#[allow(unused_imports)]
use crate::instructions::__client_accounts_resolved_signer;
#[allow(unused_imports)]
use crate::instructions::__cpi_client_accounts_resolved_signer;

// in validate():
signer.resolve(&**consensus_account)?;
consensus_account.verify_classified_signer(
    signer.classified()?,
    remaining_accounts, message, evd.as_ref(), Some(Permission::Execute),
)?;

// in handler():
let resolved_key = *signer.resolved_key()?;
```

After (1 line + clean import):
```rust
use crate::instructions::prelude::*;

// in validate():
signer.verify(
    &mut **consensus_account,
    remaining_accounts, message, evd.as_ref(), Some(Permission::Execute),
)?;

// in handler():
let resolved_key = signer.resolved_key()?;
```

### Anchor boilerplate imports

Replace the per-file `#[allow(unused_imports)]` pairs with a single glob import. `transaction_create_from_buffer.rs` already uses `use crate::instructions::*` successfully without `#[allow]` -- the glob naturally suppresses unused warnings.

Two options (either works):

**Option A** (no new file): Each instruction file uses `use crate::instructions::*;`. This is broader (pulls in all instruction re-exports) but requires zero new files and is already proven in the codebase.

**Option B** (new prelude file): Create `instructions/prelude.rs` re-exporting only the two boilerplate modules. More surgical, one extra file.

Prefer Option A for simplicity -- no new files, already proven.

### Session key duplicate check

The `create_session_key` instruction must enforce session key uniqueness at creation time. This is critical because `verify_signer` trusts that `find_signer_by_session_key` returns at most one match.

Existing checks (preserve as-is):
1. **Session key vs signer keys**: `settings.signers.find(&args.session_key).is_none()` -- prevents the session key from colliding with a signer's primary key
2. **Session key vs other session keys**: `!settings.signers.has_session_key_assigned(&args.session_key)` -- prevents the session key from shadowing another signer's session key

Self-rotation is allowed: a signer can overwrite their own active session key by setting a new one (different pubkey) without revoking first. `has_session_key_assigned` correctly permits this because the new key won't match the old one.

## Files affected

### Modified
- `state/resolved_signer.rs` -- remove `classified` OnceCell, replace `resolve()`/`classified()`/`resolved_key()` with `verify()`/`resolved_key()`
- `interface/consensus_trait.rs` -- delete `ClassifiedSigner`, `classify_signer`, `verify_classified_signer`; fold logic into `verify_signer`; rewrite `resolve_signer_key` internals
- 15 instruction files using `ResolvedSigner` -- replace 3-line dance with single `verify()` call, replace boilerplate imports with glob
- `instructions/transaction_create_from_buffer.rs` -- uses the pattern through nested `self.transaction_create.creator` field
- `instructions/transaction_buffer_close.rs` -- calls `verify_signer` directly on `ConsensusAccount` (not through `ResolvedSigner`) and calls `resolve_signer_key`; affected by `resolve_signer_key` internal rewrite but not by `ResolvedSigner` API changes

## Scope boundaries

- Sync execution paths (`transaction_execute_sync.rs`, `settings_transaction_sync.rs`) are not affected -- they don't use `ResolvedSigner` and continue using `validate_synchronous_consensus` + `resolve_signer_key`
- `create_session_key.rs` uses `ResolvedSigner` but has its own verification flow (direct external signer verification, not `verify_signer`) -- only the boilerplate import changes
- `transaction_buffer_close.rs` calls `verify_signer` directly on `ConsensusAccount` and `resolve_signer_key` for event logging -- affected by internal rewrites but not by `ResolvedSigner` API changes
- No SDK changes required -- `ResolvedSigner` is an on-chain type, the SDK sees it as a single account in the instruction
- No test changes required beyond confirming the existing test suite passes (same 3 expected failures)
