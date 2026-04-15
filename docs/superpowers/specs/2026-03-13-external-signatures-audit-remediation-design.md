# External Signatures Branch: Full Audit + Remediation

**Branch:** `feat/external-signatures`
**Date:** 2026-03-13

## Context

The `feat/external-signatures` branch implements V2 external signer support (P256/WebAuthn, Secp256k1, Ed25519External) for the Squads Smart Account program. A full audit revealed several gaps: a deserialization safety issue, stubbed session key instructions, a dead error variant, 24 orphaned V2 test files, and commented-out test suites. This spec covers fixing all of these.

## Scope

### In Scope
1. Cherry-pick deserialization safety fix from `feat/v2-signers-implementation`
2. Implement session key instructions (currently stubbed as `NotImplemented`)
3. Remove dead `PlaceholderError` variant
4. Enable and fix all 24 orphaned V2 test files
5. Un-comment disabled test blocks in programInteractionPolicy (V1 + V2)
6. Un-comment and implement externalSignerTypes.ts tests
7. Un-comment and enable sessionKeys.ts tests
8. Verify all V2 lib.rs entrypoints have test coverage

### Out of Scope
- BPF stack refactoring (no evidence of stack overflow)
- Policy PDA seed reuse (predates our branch, authored by 0xRigel)
- TODO cleanup (P256-NATIVE, performance TODOs in program_interaction.rs, clone in settings.rs)
- Authority operation V2 variants (settings_authority is always a native keypair)

---

## 1. ConsensusAccount Deserialization Safety Fix

**File:** `programs/squads_smart_account_program/src/interface/consensus.rs`

**Problem:** Lines 41 and 54 use `.unwrap()` on discriminator byte slicing. Panics on malformed account data (buffer < 8 bytes).

**Fix:** Add bounds check + replace `.unwrap()` with `.map_err()`:
```rust
// Before
let discriminator: [u8; 8] = reader[..8].try_into().unwrap();

// After
require!(
    reader.len() >= 8,
    anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch
);
let discriminator: [u8; 8] = reader[..8]
    .try_into()
    .map_err(|_| anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch)?;
```

Apply to both `try_deserialize` and `try_deserialize_unchecked`. Add `use anchor_lang::require;` import.

---

## 2. Session Key Implementation

**Files:**
- `programs/.../instructions/create_session_key.rs` (currently returns `NotImplemented`)
- `programs/.../instructions/revoke_session_key.rs` (currently returns `NotImplemented`)
- `programs/.../state/settings.rs` (needs `SetSessionKey`/`ClearSessionKey` action handling)

**Current state:** The `SessionKeyData` struct and all signer methods (`set_session_key`, `clear_session_key`, `has_active_session_key`, `is_valid_session_key`, `get_session_key_data_if_matches`) are fully implemented in `signer_v2/types/signer.rs`. The instruction handlers are stubs. Entrypoints exist in `lib.rs`.

**Implementation approach:**

### create_session_key
- Verify the parent signer (native or external via precompile/syscall)
- Validate session key is not the default pubkey
- Validate expiration is in the future and within 3-month max
- Call `set_session_key()` on the parent signer
- Realloc account if SessionKeyData increases size
- Emit event for audit trail

### revoke_session_key
- Verify the parent signer (native or external)
- Call `clear_session_key()` on the parent signer
- Emit event for audit trail

### Settings actions
- Add `SetSessionKey` and `ClearSessionKey` variants to `SettingsAction` enum if not already present
- Handle them in `modify_with_action` to allow governance-based session key management

**Reference:** `feat/v2-signers-implementation` has complete implementations in `session_key_add.rs` and `session_key_remove.rs` with signature verification via `verify_v2_context()`.

---

## 3. Remove PlaceholderError

**File:** `programs/.../errors.rs`

Remove the `PlaceholderError` variant at line 121. It is defined but never used anywhere in the codebase. Since error discriminator values are positional in Anchor, only safe to remove if it's the last variant (verify before removing). If it's not last, replace the name with a comment noting it's reserved to preserve discriminator ordering.

---

## 4. Enable 24 Orphaned V2 Test Files

**Location:** `tests/suites/v2/instructions/`

Currently only `mixedSignerSync.ts` is imported. The following 24 files need to be imported into the test index and any failures fixed:

- batchAccountsClose, batchTransactionAccountClose, cancelRealloc
- incrementAccountIndex, internalFundTransferPolicy, logEvent
- policyCreation, policyExpiration, policyUpdate
- programInteractionPolicy, removePolicy, settingsChangePolicy
- settingsTransactionAccountsClose, settingsTransactionExecute
- settingsTransactionSynchronous, smartAccountCreate
- smartAccountSetArchivalAuthority, spendingLimitPolicy
- transactionAccountsClose, transactionBufferClose
- transactionBufferCreate, transactionBufferExtend
- transactionCreateFromBuffer, transactionSynchronous

**Approach:**
1. Add imports to test index files (`index.ts` and/or `index-v2-only.ts`)
2. Build and run — collect failures
3. Fix failures iteratively (likely SDK wrapper issues, missing V2 instruction calls, account setup)
4. Each test file may need V2 SDK instruction wrappers where V1 wrappers are currently used

---

## 5. Un-comment programInteractionPolicy Tests

**Files:**
- `tests/suites/instructions/programInteractionPolicy.ts` (~49% commented out)
- `tests/suites/v2/instructions/programInteractionPolicy.ts` (same pattern)

Two test blocks are commented out:
1. "Program Interaction Policy" — basic flow
2. "Program Interaction Policy - Account Data Constraints"

Un-comment and fix. These likely depend on the noop program fixture being loaded in the validator (already documented in testing.md).

---

## 6. Un-comment + Implement externalSignerTypes.ts

**File:** `tests/suites/instructions/externalSignerTypes.ts` (30KB, fully commented out)

This file contains tests for all external signer types but has a TODO: "Add tests for actual transaction signing with external signatures + precompiles."

**Approach:**
1. Un-comment the file and import it
2. Use existing helpers from `tests/helpers/crypto.ts` (keypair generation, precompile instruction builders)
3. Use `tests/helpers/extraVerificationData.ts` for EVD serialization
4. Implement end-to-end flows: create smart account with external signer → create transaction → approve via precompile → execute
5. Test all 3 external signer types + syscall fallback for Ed25519/Secp256k1

---

## 7. Un-comment + Enable sessionKeys.ts

**File:** `tests/suites/instructions/sessionKeys.ts` (8KB, commented out in index)

Depends on session key implementation (item 2). After implementing the instruction handlers:
1. Import the test file
2. Update tests to use new session key instructions
3. Test: create session key → use session key for transaction → revoke → verify revoked key fails
4. Test expiration validation (expired keys rejected)
5. Port session key builder helpers from `v2-signers-implementation` (`deriveSignerKeyId`, `buildSessionKeyData`)

---

## 8. V2 Entrypoint Test Coverage Verification

After enabling all test files, verify every V2 entrypoint in `lib.rs` has at least one test exercising it. Cross-reference the entrypoint list with test coverage. Document any gaps.

V2 entrypoints to verify:
- create_transaction_v2, create_settings_transaction_v2
- approve_proposal_v2, reject_proposal_v2, cancel_proposal_v2
- activate_proposal_v2
- execute_transaction_v2, execute_settings_transaction_v2
- execute_transaction_sync_v2, execute_settings_transaction_sync_v2
- create_batch_v2, add_transaction_to_batch_v2, execute_batch_transaction_v2
- create_session_key, revoke_session_key
- Any other V2 variants in lib.rs

---

## Execution Order

Items are ordered by dependency:
1. **Deserialization fix** (no dependencies, foundation safety)
2. **Remove PlaceholderError** (no dependencies, trivial)
3. **Session key implementation** (needed before session key tests)
4. **Enable V2 test files** (may surface more issues)
5. **Un-comment programInteractionPolicy tests**
6. **Un-comment externalSignerTypes.ts tests**
7. **Un-comment sessionKeys.ts tests** (depends on item 3)
8. **V2 entrypoint coverage audit** (depends on all tests being enabled)

## Success Criteria

- Program builds with `anchor build -- --features=testing`
- All previously passing tests still pass
- All newly enabled tests pass
- No `NotImplemented` stubs remain for session key instructions
- No orphaned V2 test files
- Every V2 entrypoint in lib.rs has at least one test
- Expected failures remain exactly 2 (increment_account_index V1 + V2)
