# V2 External Signatures — Audit Remediations Spec

Status: **Open**
Last updated: 2026-04-18

---

## Context

V2 introduces three distinct identities where V1 had one:

| Identity | V1 | V2 |
|---|---|---|
| Raw transaction signer | ed25519 keypair | ed25519 keypair, session key, or external signer AccountInfo |
| Canonical governance key | same as above | `signer.key()` — truncated curve key for P256/secp256k1, pubkey for Ed25519External, Solana pubkey for Native |
| Lamport recipient | same as above | whoever paid rent (`rent_payer`) — may be unrelated to canonical key |

Most of the codebase was updated to use canonical keys for authorization. The
issues below are places where state layouts, close semantics, or resolution
paths still assume the old native-only identity model.

### Root cause pattern

Every issue in this document (and in PR #34) traces to the same V1 equivalence:

> **V1: proof-of-identity = operational-capability.**
> The ed25519 keypair simultaneously proves who you are, authorizes the exact
> payload, receives lamports, holds signer privilege in CPIs, and occupies a
> unique key-space.

V2 breaks this into separate concerns. Any code path that implicitly relies on
the collapsed V1 identity is a candidate for bugs:

| V1 assumption | Breaks when... | Example |
|---|---|---|
| Canonical key is a spendable Solana address | P256/secp256k1 canonical key is truncated curve bytes | Buffer `close = creator` (#1) |
| A key occupies exactly one role | Session key address added as a signer canonical key | Sync/async resolution split (Appendix) |
| Signer privilege in outer tx = privilege in inner CPI | Session key retains `is_signer` inside CPI accounts | Sync signer leak (#4, patched) |
| Signed message implicitly covers payload | External signer signs domain message, not raw tx | PR #34 fix #1 (patched) |
| `remaining_accounts` layout is static | Instructions sysvar needed at position 0 for precompiles | PR #34 fix #2 (patched) |
| User-provided counts are bounded | `num_signer` used as slice index without check | PR #34 fix #4 (patched) |

---

## Must Solve

### 1. HIGH — Buffer lifecycle has wrong refund model for V2 signers

**Files:**
- `programs/.../state/transaction_buffer.rs:10` — no `rent_collector` field
- `programs/.../instructions/transaction_buffer_close.rs:20` — `close = creator`
- `programs/.../instructions/transaction_create_from_buffer.rs:16` — `close = creator`

**Problem:**

`TransactionBuffer` stores `creator: Pubkey` (the canonical governance key).
Both close paths use Anchor's `close = creator`, which sends all lamports to
whatever `AccountInfo` is passed as `creator`.

For P256Webauthn, Secp256k1, and P256Native signers, the canonical key is
derived from non-ed25519 curve key material (`compressed_pubkey[..32]` or
`uncompressed_pubkey[..32]`). Nobody holds the ed25519 private key for that
Solana address. **Lamports sent there are permanently locked.**

Trace for a P256Webauthn signer closing their own buffer (no session key):

1. Buffer created via `create_transaction_buffer_v2` — stores
   `creator = resolved_key = Pubkey::new_from_array(compressed_pubkey[..32])`
2. Signer calls `close_transaction_buffer_v2`, passes `creator` AccountInfo
   with `key = compressed_pubkey[..32]`
3. Validate slow path: `verify_signer()` confirms external signer → OK
4. Handler body: `creator.key() == transaction_buffer.creator` → direct match → OK
5. Anchor `close = creator` sends **all buffer lamports** to
   `compressed_pubkey[..32]` — **an unspendable address**

Session keys work around this (lamports go to session key address), but the
direct path is broken. Ed25519External is unaffected (same curve as Solana).

**Remediation:**

In-place change to `TransactionBuffer` (no V2 type needed — no live buffers
exist on-chain). Add `rent_collector` field, close to it instead of `creator`.

**PENDING DECISION:** Whether buffer close keeps the `creator` authorization
check (prevents grief — attacker closing someone's mid-fill buffer) or goes
permissionless (simpler — rent always returns to `rent_collector` regardless).
Recommendation: keep creator check to prevent DoS on in-progress buffers.

```rust
pub struct TransactionBuffer {
    pub settings: Pubkey,
    pub creator: Pubkey,         // authorization identity (canonical key)
    pub rent_collector: Pubkey,  // lamport recipient (rent_payer.key() at creation)
    pub buffer_index: u8,
    pub account_index: u8,
    pub final_buffer_hash: [u8; 32],
    pub final_buffer_size: u16,
    pub buffer: Vec<u8>,
}
```

Then `close = rent_collector` in both `CloseTransactionBuffer` and
`CreateTransactionFromBuffer`, with an Anchor `address` constraint:
```rust
#[account(mut, address = transaction_buffer.rent_collector)]
pub rent_collector: AccountInfo<'info>,
```

This matches the pattern used by `Transaction`, `SettingsTransaction`, `Batch`,
`BatchTransaction`, and `Proposal` — all of which store `rent_collector`
separately and close to it.

If live buffer compatibility matters, this becomes a `TransactionBufferV2` /
new-layout problem. If not, an in-place layout change is fine (no live buffers
should persist across upgrades since they're ephemeral by design).


### 2. MEDIUM / MUST DECIDE — Buffer-close authorization: removed signers

**File:** `programs/.../instructions/transaction_buffer_close.rs:44-48`

**Problem:**

The fast path in `validate()` explicitly allows buffer closure without
current-membership verification:

```rust
// Fast path: native signer whose key directly matches stored creator.
// No membership check — allows removed members to reclaim buffer rent.
if self.creator.is_signer && self.creator.key() == self.transaction_buffer.creator {
    return Ok(());
}
```

This means a removed signer can still close their old buffers and recover rent.

**Decision needed:**

- **If intended:** Document the policy, add a test proving a removed signer can
  close their buffer, and add a test proving they CANNOT extend or finalize it.
- **If not intended:** Remove the fast path and always require current
  `verify_signer` membership. Note that with the `rent_collector` fix from
  issue #1, the removed signer no longer needs to be the lamport recipient
  anyway — the original `rent_payer` gets the refund regardless.

---

## Should Solve

### 7. HIGH — Policy sync path signer stripping uses canonical keys (not raw keys)

**File:** `programs/.../state/policies/implementations/program_interaction.rs:1464-1468`

**Problem:**

```rust
let policy_signer_keys: Vec<Pubkey> = args
    .policy_signers
    .iter()
    .map(|signer| signer.key())  // canonical keys from SmartAccountSigner
    .collect();

let executable_message = SynchronousTransactionMessage::new_validated(
    &settings_key,
    &smart_account_pubkey,
    &policy_signer_keys,  // passed as outer_signer_keys
    ...
);
```

This is the **same bug** that was patched in the settings path (#4). The
settings fix collects **raw account keys**:
```rust
let outer_signer_keys: Vec<Pubkey> = ctx.remaining_accounts[..args.num_signers as usize]
    .iter()
    .map(|acc| *acc.key)  // raw AccountInfo keys
    .collect();
```

**Exploitable NOW.** Policies support session keys through the Consensus trait:
`find_signer_by_session_key` (`consensus_trait.rs:42-43`) is a default method
that works on ANY consensus account. Policy signers include `SessionKeyData`
in their data structure. Session key data can be populated at `PolicyCreate`
time (the `signers` field in the action payload is client-serialized).

**Attack path:**
1. Policy created with external signer that has active session key S in data
2. Session key holder sends sync transaction on the policy
3. `validate_synchronous_consensus` Phase 1: session key authenticates via
   `find_signer_by_session_key(S, now)` → returns parent signer
4. Policy execution: `policy_signer_keys` uses `signer.key()` (canonical keys)
   → S is NOT in the list → `is_signer` NOT stripped in inner CPI
5. Session key retains signer privilege inside the executed transaction

**Remediation:** Same fix as the settings path — collect raw account keys
from the accounts array passed to the policy execution, not canonical keys
from the signer objects.

### 8. MEDIUM — Session key create/revoke only works on Settings, not Policies (FIXED)

Both `create_session_key.rs` and `revoke_session_key.rs` now use
`InterfaceAccount<'info, ConsensusAccount>`, supporting both Settings and
Policy accounts.

### 9. LOW — MigrateToV2 on already-V2 account invalidates all proposals

**File:** `programs/.../state/settings.rs:738-741`

**Problem:**

```rust
SettingsAction::MigrateToV2 => {
    self.signers.force_v2();
    self.invalidate_prior_transactions();
}
```

`force_v2()` is a no-op when signers are already V2 (guarded by `if let Self::V1`).
But `invalidate_prior_transactions()` runs unconditionally, setting
`stale_transaction_index = transaction_index` — nuking all current proposals.

A governance proposal containing `MigrateToV2` on an already-V2 account
would silently invalidate every in-flight proposal with no visible effect.

**Remediation:**

```rust
SettingsAction::MigrateToV2 => {
    if matches!(self.signers, SmartAccountSignerWrapper::V1(_)) {
        self.signers.force_v2();
        self.invalidate_prior_transactions();
    }
}
```

### 10. HIGH — `add_signer` does not check new signer key against existing session keys

**File:** `programs/.../state/signer_v2/wrapper.rs:290-299`

**Problem:**

`create_session_key` prevents forward collisions — it checks that the session
key pubkey does not collide with an existing signer key or existing session key.
However, `add_signer` only checks `has_duplicate_truncated_key` and
`has_duplicate_public_key`. It does NOT check whether the new signer's
`.key()` collides with any existing signer's active session key.

**Attack path:**
1. External signer A creates a session key SK on a consensus account
2. A settings transaction adds a new signer B whose `.key()` equals SK
3. Session key SK now matches both: signer A's session key AND signer B's key
4. `verify_signer` / `resolve_signer_key` iterates signers — first match wins
5. Depending on iteration order, signer B authenticates as signer A (inheriting
   A's permissions) or signer A's session key resolves to signer B

**Remediation:**

In `SmartAccountSignerWrapper::add_signer()`, add:
```rust
require!(
    !self.has_session_key_assigned(&signer.key()),
    crate::errors::SmartAccountError::DuplicateSigner
);
```

### 11. MEDIUM — Sync path does not protect consensus account from inner CPI writes

**File:** `programs/.../utils/synchronous_transaction_message.rs:40-55`

**Problem:**

The sync path makes the `settings_key` non-writable in inner CPI (line 50-52),
but when executing via a Policy consensus account, the policy account key is
NOT similarly protected. If the policy account's pubkey appears in the inner
CPI account set as writable, the CPI target program could modify the policy
state mid-execution.

The async path protects the proposal via `protected_accounts`. The sync path
has no equivalent mechanism for the consensus account.

**DISCUSS WITH AUDITORS:** The natural fix is to strip `is_writable` for the
consensus account key in inner CPI (same as `settings_key`). However, this
may break legitimate cases where the inner transaction needs to write to the
consensus account — e.g., passkey counter updates or other state mutations
that occur during execution. The Solana runtime borrow checker already prevents
the SAME account from being mutably borrowed twice in one call stack, which
protects the current consensus account. But a DIFFERENT consensus account
(e.g., a Policy when the outer is Settings) would not be protected.

Need auditor input on:
1. Is the runtime borrow checker sufficient protection for the current account?
2. Should we strip writes for the consensus account key anyway and handle
   counter updates separately (before/after CPI)?
3. Are there concrete attack paths via a different consensus account in inner CPI?

### 13. MEDIUM — `resolve_signer_key` does not check session key expiration

**File:** `programs/.../interface/consensus_trait.rs:52-80`

**Problem:**

`resolve_signer_key` matches session keys via `get_session_key_data_if_matches`
which explicitly does NOT check expiration. An expired session key can still
resolve to the parent signer's canonical key.

The security-critical path `verify_signer` DOES check expiration (line 183-185),
so authentication is not bypassed. However, `resolve_signer_key` is called in:
- `close_transaction_buffer_v2` — an expired session key could authorize buffer
  closure (the `validate()` call uses `verify_signer` which catches expiration,
  but the handler then re-resolves via `resolve_signer_key` for the creator match)
- Event logging in sync paths — expired session keys could incorrectly map to
  parent signers in emitted events

**Remediation:**

Add a `current_timestamp` parameter to `resolve_signer_key` and check expiration:
```rust
pub fn resolve_signer_key(&self, signer_key: Pubkey, is_signer: bool, now: u64) -> Result<Pubkey> {
    // ... existing logic ...
    if let Some(session_key_data) = signer.get_session_key_data_if_matches(&signer_key) {
        if session_key_data.expiration > now {
            return Ok(signer.key());
        }
    }
}
```

### 14. LOW — Cancel realloc uses signer as rent payer (fails for external signers)

**File:** `programs/.../instructions/proposal_vote.rs:234-237`

**Problem:**

`cancel_proposal_inner` passes `signer.to_account_info()` as the rent payer for
`realloc_if_needed`. For external signers, this AccountInfo is not a native
Solana signer and may not have lamports — the `system_program::transfer` CPI
fails.

Realloc is needed when the proposal account must grow to accommodate more cancel
votes (e.g., signers were added since proposal creation).

**Remediation:**

Add an optional `rent_payer: Option<Signer<'info>>` to the `VoteOnProposal`
struct (same pattern as `SyncSettingsTransaction`). Use it when present for
realloc, falling back to the signer's AccountInfo when it's a native signer:

```rust
let payer = ctx.accounts.rent_payer
    .as_ref()
    .map(|p| p.to_account_info())
    .or_else(|| if signer.is_native() { Some(signer.to_account_info()) } else { None });
```

### 15. MEDIUM — `remaining_accounts.len()` truncated to u8 in sync message hash

**File:** `programs/.../state/signer_v2/precompile/messages.rs:219,247`

**Problem:**

Both `create_sync_transaction_message` and `create_sync_transaction_legacy_message`
cast `remaining_accounts.len()` to `u8`:
```rust
hasher.hash(&[remaining_accounts.len() as u8]);
```

If `remaining_accounts.len()` exceeds 255, this silently wraps (256 → 0, 257 → 1).
Currently unexploitable due to Solana tx size limits (~35-40 accounts max), but
a latent design flaw if account size limits change.

**Remediation:**

Add an explicit guard or widen the hash:
```rust
require!(remaining_accounts.len() <= u8::MAX as usize, SmartAccountError::InvalidTransactionMessage);
hasher.hash(&[remaining_accounts.len() as u8]);
```

---

## Recently Fixed — Needs Regression Coverage

### 4. Sync session-key signer leak (PATCHED)

**File:** `programs/.../utils/synchronous_transaction_message.rs:16`

**What was fixed:**

Previously, `new_validated()` stripped `is_signer` from inner CPI accounts by
matching against `consensus_account_signers.iter().any(|signer| &signer.key() == account.key)`.
This used **canonical signer keys**. A session key's raw address would never
match any canonical key, so it would retain `is_signer = true` in inner CPI
calls — allowing it to impersonate itself as a signer inside the executed
transaction.

The fix changed the stripping to use `outer_signer_keys` — the raw account
keys of all outer signers collected at `transaction_execute_sync.rs:203`:

```rust
let outer_signer_keys: Vec<Pubkey> = ctx.remaining_accounts[..args.num_signers as usize]
    .iter()
    .map(|acc| *acc.key)
    .collect();
```

And in `new_validated`:
```rust
} else if account.is_signer && outer_signer_keys.iter().any(|key| key == account.key) {
    account_info.is_signer = false;
}
```

This correctly strips session key signer privilege when the same address
appears in the inner account set.

**Required regression tests:**

1. A session key can authenticate the outer sync transaction (positive case)
2. If that same raw session key address is duplicated into the inner payload
   accounts, it is NOT seen as an inner signer (the critical negative case)
3. A canonical signer key duplicated into inner accounts is also stripped
   (baseline behavior preserved)

This class of bug is easy to reintroduce. The regression test is mandatory.

---

## Coverage Gaps

### 5. Buffer V2 end-to-end coverage

The indexed test suites cover native signer paths. There is a standalone V2
close smoke test (`tests/standalone/test-close-buffer-v2.ts`), but the main
suite needs full external-signer buffer lifecycle coverage:

- [ ] External signer creates buffer via `create_transaction_buffer_v2`
- [ ] External signer extends buffer via `extend_transaction_buffer_v2`
- [ ] External signer closes buffer via `close_transaction_buffer_v2`
- [ ] External signer finalizes via `create_transaction_from_buffer_v2`
- [ ] Session key performs each of the above operations
- [ ] Revoked session key is rejected for each operation
- [ ] Expired session key is rejected for each operation
- [ ] After `rent_collector` fix: verify lamports go to `rent_payer`, not canonical key

---

## Appendix: Sync/Async Resolution Inconsistency (Informational)

During analysis we identified that the sync path
(`validate_synchronous_consensus` at `context_validation.rs:88-91`) and async
path (`verify_signer` at `consensus_trait.rs:165-199`) resolve native tx
signers through **different** logic:

**Sync (Phase 1):**
```rust
let member = consensus_account
    .is_signer_v2(signer_key)         // matches ANY signer type by canonical key
    .or_else(|| consensus_account.find_signer_by_session_key(signer_key, now))
```

**Async:**
```rust
for signer in self.signers_v2().into_iter() {
    match signer.signer_type() {
        SignerType::Native => { /* match by canonical key */ }
        _ => { /* match by session key DATA only */ }
    }
}
```

The sync path's `is_signer_v2()` is type-blind — it matches an Ed25519External
signer by canonical key even when the account signed natively. The async path
only matches non-native signers via their session key data.

**Exploitation requires** a signer whose canonical key collides with an active
session key address. `create_session_key` prevents forward collisions (session
key ≠ any signer key), but `add_signer` does NOT prevent reverse collisions
(new signer key ≠ any active session key). If such a collision exists, the
session key holder is resolved as the WRONG signer (with potentially different
permissions) in sync transactions — a governance threshold bypass.

**Practical risk is low** because adding the colliding signer requires threshold
approval. However, governance members would have no way to notice the collision.

**Recommended hardening (defense-in-depth):**

1. `add_signer` should reject if `new_signer.key()` matches any active session key:
   ```rust
   require!(
       !self.has_session_key_assigned(&signer.key()),
       SmartAccountError::DuplicateSigner
   );
   ```
2. Optionally, align the sync Phase 1 resolution to use the same type-aware
   logic as `verify_signer` (only match Native signers by key, match non-native
   signers exclusively via session key data).

---

## Discussion / Needs Analysis

### 16. Settings sync message does not bind `remaining_accounts`

**File:** `programs/.../state/signer_v2/precompile/messages.rs:264-276`
**File:** `programs/.../instructions/settings_transaction_sync.rs:66-72`

**Observation:**

`create_sync_settings_message` hashes only:
`"sync_settings_tx_v2" || consensus_key || tx_index || payload_hash`

Unlike `create_sync_transaction_message` (which hashes each remaining account's
key and `is_writable`), the settings sync message does NOT bind
`remaining_accounts` into the hash.

The settings sync path does not execute arbitrary CPI — it calls
`settings.modify_with_action()` directly. However, `remaining_accounts` ARE
passed to `modify_with_action` and used for operations like spending limit
account initialization. A relayer could potentially substitute different
`remaining_accounts` (e.g., a different spending limit PDA or rent payer) while
reusing a valid external signer signature, since the signature only covers the
`actions` payload, not the execution context.

**Mitigating factors:**
- Spending limit PDAs are program-derived and validated by Anchor account
  constraints
- The `actions` themselves are hash-bound (via `payload_hash`)
- No CPI is executed, limiting the attack surface

**Question:** Is the PDA constraint sufficient, or should `remaining_accounts`
be bound into the message for defense-in-depth? The transaction sync path
already does this — the asymmetry is worth examining.

---

## Appendix B: Investigated and Ruled Out

These were flagged during scanning but confirmed NOT bugs after tracing:

### Execute messages don't include payload hash

`create_execute_transaction_message`, `create_execute_settings_transaction_message`,
and `create_batch_execute_transaction_message` only include transaction key and
index — no payload hash.

**Not a bug.** The Transaction, SettingsTransaction, and BatchTransaction
accounts are immutable PDAs. Content is set at creation (where the payload IS
hashed into the creation message) and never modified. Signing over the
transaction key implicitly binds to the immutable content.

### Raw key vs resolved key in message construction

Several message builders use `signer.key()` (raw) instead of
`signer.resolved_key()`. E.g. `batch_execute_transaction.rs:87`,
`batch_create.rs:65`, `increment_account_index.rs:113`.

**Not a bug for current signer types.** The message is only cryptographically
verified for external signers, where raw key = canonical key (same value).
Session keys don't verify message content (native `is_signer` check only).
Native signers don't do external verification. No current code path produces a
mismatch. However, if a future signer type has raw key ≠ canonical key AND
requires message verification, this would break.

### SpendingLimit `rent_collector` is unconstrained

`authority_spending_limit_remove.rs:27` uses `close = rent_collector` where
`rent_collector` is `CHECK: can be any account`.

**Intentional.** This instruction is only callable by the `settings_authority`
(a trusted admin for controlled smart accounts). The authority deliberately
chooses where rent goes. Not a V2 identity issue.

### Signer key extraction `.try_into().unwrap()` in `signer.rs`

`signer.rs:56,59,65` do `compressed_pubkey[..32].try_into().unwrap()`.

**Not a bug.** `compressed_pubkey` is `[u8; 33]` and `uncompressed_pubkey` is
`[u8; 65]` — fixed-size fields from Borsh deserialization. Slicing `[..32]`
on a 33-byte or 65-byte array always succeeds. These are not user-provided
dynamic slices.
