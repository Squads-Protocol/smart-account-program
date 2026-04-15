# Sync Message Remediation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix three security vulnerabilities in sync transaction message signing: missing remaining_accounts commitment, missing account_index, and shared discriminator slug.

**Architecture:** All fixes are in the message construction layer (`messages.rs`) and its three sync callers. The message hash is extended to bind external signatures to the exact execution context. SDK test helpers are updated to match.

**Tech Stack:** Rust (Anchor), TypeScript (tests), SHA-256 hashing

---

## Vulnerabilities

1. **Remaining accounts not in signed message** — A relayer can swap CPI destination accounts after obtaining valid external signatures. The `payload_hash` covers compiled instructions but not the account table those instructions operate on.

2. **`account_index` not in signed message** — A relayer can swap which vault PDA signs CPIs, redirecting signer authority to a higher-value vault.

3. **Shared discriminator slug** — `sync_transaction` and `sync_settings_transaction` both use `"squads-sync"`. A signature for one could theoretically replay against the other if payload bytes happen to deserialize validly under both.

All three are **sync-path only**. Async transactions store accounts on-chain and go through proposal/vote governance.

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `programs/.../state/signer_v2/precompile/messages.rs` | Modify | Split `create_sync_consensus_message` into 3 distinct functions |
| `programs/.../instructions/transaction_execute_sync.rs` | Modify | Pass `account_index` + `remaining_accounts` to new message fn |
| `programs/.../instructions/transaction_execute_sync_legacy.rs` | Modify | Pass `account_index` + `remaining_accounts` to new message fn |
| `programs/.../instructions/settings_transaction_sync.rs` | Modify | Use new settings-specific message fn |
| `tests/suites/v2/instructions/externalSignerSecurity.ts` | Modify | Update `buildSyncConsensusMessage` to match new hash |
| `tests/suites/v2/instructions/externalSignerNoncePersistence.ts` | Modify | Update sync message construction |
| `tests/suites/v2/instructions/mixedSignerSync.ts` | Modify | Update sync message construction |

---

### Task 1: Split sync message functions in `messages.rs`

**Files:**
- Modify: `programs/squads_smart_account_program/src/state/signer_v2/precompile/messages.rs:218-230`

- [ ] **Step 1: Replace `create_sync_consensus_message` with three distinct functions**

Replace the existing function (lines 218-230):

```rust
/// Create message for synchronous consensus verification
///
/// This is used by external signers to prove they're authorizing a sync transaction.
/// The payload_hash binds the signature to the specific instructions being executed,
/// preventing a malicious transaction assembler from substituting a different payload.
///
/// Format: hash("squads-sync" || consensus_account_key || transaction_index || payload_hash)
pub fn create_sync_consensus_message(
    consensus_account_key: &Pubkey,
    transaction_index: u64,
    payload_hash: &[u8; 32],
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"squads-sync");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());
    hasher.hash(payload_hash);

    hasher
}
```

With these three functions:

```rust
/// Create message for synchronous transaction execution (v2 layout).
///
/// External signers commit to the exact execution context: which vault signs,
/// which accounts are passed, and what instructions run. This prevents a relayer
/// from swapping remaining_accounts or account_index after collecting signatures.
///
/// Format: hash("sync_transaction_v2" || consensus_key || tx_index || account_index
///              || num_accounts || for each: (key || is_writable) || payload_hash)
pub fn create_sync_transaction_message(
    consensus_account_key: &Pubkey,
    transaction_index: u64,
    account_index: u8,
    remaining_accounts: &[AccountInfo],
    payload_hash: &[u8; 32],
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"sync_transaction_v2");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());
    hasher.hash(&[account_index]);
    hasher.hash(&[remaining_accounts.len() as u8]);
    for acc in remaining_accounts {
        hasher.hash(acc.key.as_ref());
        hasher.hash(&[acc.is_writable as u8]);
    }
    hasher.hash(payload_hash);

    hasher
}

/// Create message for synchronous transaction execution (legacy layout).
///
/// Same security properties as the v2 variant but uses the legacy discriminator
/// to avoid breaking existing legacy sync callers.
///
/// Format: hash("sync_transaction_legacy" || consensus_key || tx_index || account_index
///              || num_accounts || for each: (key || is_writable) || payload_hash)
pub fn create_sync_transaction_legacy_message(
    consensus_account_key: &Pubkey,
    transaction_index: u64,
    account_index: u8,
    remaining_accounts: &[AccountInfo],
    payload_hash: &[u8; 32],
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"sync_transaction_legacy");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());
    hasher.hash(&[account_index]);
    hasher.hash(&[remaining_accounts.len() as u8]);
    for acc in remaining_accounts {
        hasher.hash(acc.key.as_ref());
        hasher.hash(&[acc.is_writable as u8]);
    }
    hasher.hash(payload_hash);

    hasher
}

/// Create message for synchronous settings transaction.
///
/// Settings sync doesn't execute CPI instructions against a vault, so there's
/// no account_index or remaining_accounts to commit to. The payload_hash covers
/// the exact settings actions being applied.
///
/// Format: hash("sync_settings_tx_v2" || consensus_key || tx_index || payload_hash)
pub fn create_sync_settings_message(
    consensus_account_key: &Pubkey,
    transaction_index: u64,
    payload_hash: &[u8; 32],
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"sync_settings_tx_v2");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());
    hasher.hash(payload_hash);

    hasher
}
```

- [ ] **Step 2: Delete the dead `create_signature_message` function**

Remove lines 7-19 (the unused generic message function):

```rust
// DELETE THIS ENTIRE FUNCTION — it's dead code, never called
pub fn create_signature_message(
    smart_account: &Pubkey,
    operation_type: &[u8],
    operation_data: &[u8],
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"squads-v2");
    hasher.hash(smart_account.as_ref());
    hasher.hash(operation_type);
    hasher.hash(operation_data);

    hasher
}
```

- [ ] **Step 3: Verify it compiles (will fail — callers not updated yet)**

Run: `cargo check -p squads-smart-account-program --features=testing 2>&1 | grep "error"`
Expected: Errors about `create_sync_consensus_message` not found in callers.

---

### Task 2: Update `transaction_execute_sync.rs` (v2 sync)

**Files:**
- Modify: `programs/squads_smart_account_program/src/instructions/transaction_execute_sync.rs:13,131-140`

- [ ] **Step 1: Update the import**

Replace:
```rust
use crate::state::signer_v2::precompile::create_sync_consensus_message;
```

With:
```rust
use crate::state::signer_v2::precompile::create_sync_transaction_message;
```

- [ ] **Step 2: Update the message construction in `validate()`**

Replace lines 131-140:
```rust
        // Build message for external signer verification.
        // Hash the payload so external signers commit to the exact instructions being executed.
        let payload_bytes = args.payload.try_to_vec()
            .map_err(|_| SmartAccountError::InvalidPayload)?;
        let payload_hash = hash(&payload_bytes);
        let message = create_sync_consensus_message(
            &consensus_account.key(),
            consensus_account.transaction_index(),
            &payload_hash.to_bytes(),
        );
```

With:
```rust
        // Build message for external signer verification.
        // Hash commits to payload + account_index + remaining_accounts, preventing
        // a relayer from swapping CPI accounts or vault index after collecting signatures.
        let payload_bytes = args.payload.try_to_vec()
            .map_err(|_| SmartAccountError::InvalidPayload)?;
        let payload_hash = hash(&payload_bytes);
        let message = create_sync_transaction_message(
            &consensus_account.key(),
            consensus_account.transaction_index(),
            args.account_index,
            &remaining_accounts[accounts_start..],
            &payload_hash.to_bytes(),
        );
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p squads-smart-account-program --features=testing 2>&1 | grep "error"`
Expected: Errors only from the two remaining callers (legacy + settings), not this file.

---

### Task 3: Update `transaction_execute_sync_legacy.rs`

**Files:**
- Modify: `programs/squads_smart_account_program/src/instructions/transaction_execute_sync_legacy.rs:13,68-75`

- [ ] **Step 1: Update the import**

Replace:
```rust
    state::signer_v2::precompile::create_sync_consensus_message,
```

With:
```rust
    state::signer_v2::precompile::create_sync_transaction_legacy_message,
```

- [ ] **Step 2: Read the full validate function to find the remaining_accounts slice**

Read `transaction_execute_sync_legacy.rs` from line 45 to understand what `remaining_accounts` variable is available.

- [ ] **Step 3: Update the message construction**

Replace lines 68-75:
```rust
        // Build message for external signer verification.
        // Hash the instructions payload so external signers commit to the exact instructions.
        let payload_hash = hash(&args.instructions);
        let message = create_sync_consensus_message(
            &consensus_account.key(),
            consensus_account.transaction_index(),
            &payload_hash.to_bytes(),
        );
```

With:
```rust
        // Build message for external signer verification.
        // Hash commits to payload + account_index + remaining_accounts.
        let payload_hash = hash(&args.instructions);
        let message = create_sync_transaction_legacy_message(
            &consensus_account.key(),
            consensus_account.transaction_index(),
            args.account_index,
            &remaining_accounts[args.num_signers as usize..],
            &payload_hash.to_bytes(),
        );
```

Note: The legacy handler passes `remaining_accounts` directly to `validate()`. The accounts after the signers are the execution accounts. Verify by reading the full function — `remaining_accounts` may need to account for the optional instructions sysvar offset (same pattern as the v2 handler). Adjust the slice start accordingly.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p squads-smart-account-program --features=testing 2>&1 | grep "error"`
Expected: Error only from settings_transaction_sync.rs.

---

### Task 4: Update `settings_transaction_sync.rs`

**Files:**
- Modify: `programs/squads_smart_account_program/src/instructions/settings_transaction_sync.rs:4,69`

- [ ] **Step 1: Update the import**

In the long import line (line 4), replace:
```rust
state::signer_v2::precompile::create_sync_consensus_message
```

With:
```rust
state::signer_v2::precompile::create_sync_settings_message
```

- [ ] **Step 2: Update the message construction**

Replace lines 64-73:
```rust
        // Build message for external signer verification.
        // Hash the actions payload so external signers commit to the exact settings changes.
        let actions_bytes = args.actions.try_to_vec()
            .map_err(|_| SmartAccountError::InvalidPayload)?;
        let payload_hash = hash(&actions_bytes);
        let message = create_sync_consensus_message(
            &consensus_account.key(),
            consensus_account.transaction_index(),
            &payload_hash.to_bytes(),
        );
```

With:
```rust
        // Build message for external signer verification.
        // Hash the actions payload so external signers commit to the exact settings changes.
        let actions_bytes = args.actions.try_to_vec()
            .map_err(|_| SmartAccountError::InvalidPayload)?;
        let payload_hash = hash(&actions_bytes);
        let message = create_sync_settings_message(
            &consensus_account.key(),
            consensus_account.transaction_index(),
            &payload_hash.to_bytes(),
        );
```

- [ ] **Step 3: Verify full program compiles**

Run: `cargo check -p squads-smart-account-program --features=testing 2>&1 | grep "error"`
Expected: No errors.

---

### Task 5: Update the `precompile/mod.rs` exports

**Files:**
- Modify: `programs/squads_smart_account_program/src/state/signer_v2/precompile/mod.rs`

- [ ] **Step 1: Check current exports and update**

Read `precompile/mod.rs` to find the re-export of `create_sync_consensus_message`. Replace it with the three new functions:

```rust
// Remove:
create_sync_consensus_message,
// Add:
create_sync_transaction_message,
create_sync_transaction_legacy_message,
create_sync_settings_message,
```

Also remove `create_signature_message` from exports if present.

- [ ] **Step 2: Full build**

Run: `avm use 0.29.0 && anchor build -- --features=testing 2>&1 | tail -5`
Expected: Compiles with only warnings, no errors.

- [ ] **Step 3: Commit**

```bash
git add programs/
git commit -m "fix: bind sync signatures to remaining_accounts, account_index, and unique discriminators

Addresses three sync-path vulnerabilities:
1. remaining_accounts now included in signed hash (prevents account swap)
2. account_index now included in signed hash (prevents vault swap)
3. Split 'squads-sync' into 'sync_transaction_v2', 'sync_transaction_legacy',
   and 'sync_settings_tx_v2' (prevents cross-instruction replay)"
```

---

### Task 6: Update test helpers — `externalSignerSecurity.ts`

**Files:**
- Modify: `tests/suites/v2/instructions/externalSignerSecurity.ts:38-58`

- [ ] **Step 1: Update `buildSyncConsensusMessage` to match new hash format**

Replace the current function (lines 38-58):

```typescript
function buildSyncConsensusMessage(
  settingsPda: PublicKey,
  transactionIndex: bigint,
  nextNonce: bigint,
  payloadHash: Uint8Array
): Uint8Array {
  const txIndexBytes = Buffer.alloc(8);
  txIndexBytes.writeBigUInt64LE(transactionIndex);
  const nonceBytes = Buffer.alloc(8);
  nonceBytes.writeBigUInt64LE(nextNonce);

  return sha256(
    Buffer.concat([
      Buffer.from("squads-sync", "utf-8"),
      settingsPda.toBuffer(),
      txIndexBytes,
      Buffer.from(payloadHash),
      nonceBytes,
    ])
  );
}
```

With:

```typescript
/** Build the sync transaction message matching on-chain create_sync_transaction_message + nonce.
 *  Binds signature to: discriminator, consensus key, tx index, account_index,
 *  remaining accounts (key + is_writable), payload hash, and nonce. */
function buildSyncTransactionMessage(
  consensusKey: PublicKey,
  transactionIndex: bigint,
  accountIndex: number,
  remainingAccounts: { pubkey: PublicKey; isWritable: boolean }[],
  nextNonce: bigint,
  payloadHash: Uint8Array
): Uint8Array {
  const txIndexBytes = Buffer.alloc(8);
  txIndexBytes.writeBigUInt64LE(transactionIndex);
  const nonceBytes = Buffer.alloc(8);
  nonceBytes.writeBigUInt64LE(nextNonce);

  const accountBuffers: Buffer[] = [];
  for (const acc of remainingAccounts) {
    accountBuffers.push(acc.pubkey.toBuffer());
    accountBuffers.push(Buffer.from([acc.isWritable ? 1 : 0]));
  }

  return sha256(
    Buffer.concat([
      Buffer.from("sync_transaction_v2", "utf-8"),
      consensusKey.toBuffer(),
      txIndexBytes,
      Buffer.from([accountIndex]),
      Buffer.from([remainingAccounts.length]),
      ...accountBuffers,
      Buffer.from(payloadHash),
      nonceBytes,
    ])
  );
}
```

- [ ] **Step 2: Update all call sites in the file**

Find every call to `buildSyncConsensusMessage(...)` and update to pass `accountIndex` and `remainingAccounts`. The exact accounts depend on what each test constructs — read the test to identify the accounts array being passed as remaining_accounts to the sync transaction instruction, then pass the same accounts (after stripping signers and sysvar) to the message builder.

- [ ] **Step 3: Run the test to verify**

Run: `npx mocha --node-option require=ts-node/register --extension ts -t 1000000 tests/suites/v2/instructions/externalSignerSecurity.ts`
Expected: All tests pass.

---

### Task 7: Update test helpers — `externalSignerNoncePersistence.ts`

**Files:**
- Modify: `tests/suites/v2/instructions/externalSignerNoncePersistence.ts:36-57`

- [ ] **Step 1: Update the sync message builder**

Apply the same pattern as Task 6 — replace `buildSyncConsensusMessage` with `buildSyncTransactionMessage` using the new hash format with `accountIndex` and `remainingAccounts`.

- [ ] **Step 2: Update all call sites**

Same approach — read each test to identify which remaining accounts are passed, strip signers/sysvar, pass to builder.

- [ ] **Step 3: Run the test**

Run: `npx mocha --node-option require=ts-node/register --extension ts -t 1000000 tests/suites/v2/instructions/externalSignerNoncePersistence.ts`
Expected: All tests pass.

---

### Task 8: Update test helpers — `mixedSignerSync.ts`

**Files:**
- Modify: `tests/suites/v2/instructions/mixedSignerSync.ts:228-243`

- [ ] **Step 1: Update the inline sync message construction**

Replace lines 228-243:

```typescript
    // sha256("squads-sync" || settings_key || transaction_index_le || payload_hash || next_nonce_le)
    const transactionIndex = BigInt(settings.transactionIndex.toString());
    const transactionIndexBytes = Buffer.alloc(8);
    transactionIndexBytes.writeBigUInt64LE(transactionIndex);
    const nonceBytes = Buffer.alloc(8);
    nonceBytes.writeBigUInt64LE(BigInt(1)); // next_nonce = current(0) + 1
    const hashedMessage = sha256(
      Buffer.concat([
        Buffer.from("squads-sync", "utf-8"),
        settingsPda.toBuffer(),
        transactionIndexBytes,
        Buffer.from(payloadHash),
        nonceBytes,
      ])
    );
```

With the new format including `accountIndex` and `remainingAccounts`:

```typescript
    // sha256("sync_transaction_v2" || settings_key || tx_index || account_index
    //        || num_accounts || for each: (key || is_writable) || payload_hash || nonce)
    const transactionIndex = BigInt(settings.transactionIndex.toString());
    const transactionIndexBytes = Buffer.alloc(8);
    transactionIndexBytes.writeBigUInt64LE(transactionIndex);
    const nonceBytes = Buffer.alloc(8);
    nonceBytes.writeBigUInt64LE(BigInt(1)); // next_nonce = current(0) + 1

    // Build account commitment — remaining_accounts after signers + sysvar
    const executionAccounts = [/* the accounts passed after signers in remaining_accounts */];
    const accountBuffers: Buffer[] = [];
    for (const acc of executionAccounts) {
      accountBuffers.push(acc.pubkey.toBuffer());
      accountBuffers.push(Buffer.from([acc.isWritable ? 1 : 0]));
    }

    const hashedMessage = sha256(
      Buffer.concat([
        Buffer.from("sync_transaction_v2", "utf-8"),
        settingsPda.toBuffer(),
        transactionIndexBytes,
        Buffer.from([0]), // account_index
        Buffer.from([executionAccounts.length]),
        ...accountBuffers,
        Buffer.from(payloadHash),
        nonceBytes,
      ])
    );
```

Note: Read the full test to identify the exact `executionAccounts` array. It will be the accounts passed as remaining_accounts to the sync instruction, minus the signers and optional instructions sysvar.

- [ ] **Step 2: Run the test**

Run: `npx mocha --node-option require=ts-node/register --extension ts -t 1000000 tests/suites/v2/instructions/mixedSignerSync.ts`
Expected: All tests pass.

---

### Task 9: Full test suite + regenerate IDL

- [ ] **Step 1: Regenerate IDL**

Run: `avm use 0.29.0 && anchor build -- --features=testing`
Expected: Builds successfully.

- [ ] **Step 2: Run full V2 test suite**

Follow the test procedure from memory (switch to solana 3.0.0, start validator with fixtures, run mocha).
Expected: Only the 3 known `increment_account_index` failures.

- [ ] **Step 3: Commit tests + IDL**

```bash
git add tests/ sdk/smart-account/idl/
git commit -m "test: update sync message construction to match new hash format"
```
