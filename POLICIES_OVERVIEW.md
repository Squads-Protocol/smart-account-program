# Policy System Overview

The policy system provides an alternative consensus mechanism to full smart account governance. Policies allow pre-approved, parameterized actions that can be executed with reduced overhead compared to the full proposal/voting process.

## Core Architecture

### Consensus Interface

Both smart account settings and policies implement the same `Consensus` trait, allowing them to be used interchangeably for transaction validation. The `ConsensusAccount` enum wraps both types and provides unified access to consensus operations.

**File**: `programs/squads_smart_account_program/src/interface/consensus.rs`

```rust
#[derive(Clone)]
pub enum ConsensusAccount {
    Settings(Settings),
    Policy(Policy),
}
```

### Policy Account Structure

**File**: `programs/squads_smart_account_program/src/state/policies/policy_core/policy.rs`

```rust
pub struct Policy {
    pub settings: Pubkey,                    // Parent smart account
    pub seed: u64,                          // Unique policy identifier
    pub transaction_index: u64,             // Latest transaction number
    pub stale_transaction_index: u64,       // Staleness boundary
    pub signers: Vec<SmartAccountSigner>,   // Members with permissions
    pub threshold: u16,                     // Approval threshold
    pub time_lock: u32,                     // Execution delay (seconds)
    pub policy_state: PolicyState,          // Type-specific configuration
    pub start: i64,                         // Activation timestamp
    pub expiration: Option<PolicyExpiration>, // Expiration rules
}
```

### Stale Transaction Protection

Policies use the same stale transaction protection as smart account settings. When signers, threshold, or timelock change, all pending transactions become stale and cannot be executed.

### Permission System

Each policy signer has granular permissions:
- **Initiate**: Create transactions/proposals
- **Vote**: Approve/reject/cancel proposals
- **Execute**: Execute approved transactions

## Policy Lifecycle

### Policy Creation

Policies are created through `SettingsAction::PolicyCreate` in a settings transaction:

**File**: `programs/squads_smart_account_program/src/state/settings_transaction.rs`

```rust
SettingsAction::PolicyCreate {
    policy_seed: u64,
    policy_signers: Vec<SmartAccountSigner>,
    policy_threshold: u16,
    policy_time_lock: u32,
    policy_creation_payload: PolicyCreationPayload,
    policy_start: i64,
    policy_expiration: Option<PolicyExpirationArgs>,
}
```

### Policy Updates

Updates use `SettingsAction::PolicyUpdate` and require full smart account governance. Updates invalidate prior transactions via `invalidate_prior_transactions()`.

### Policy Deletion

Policies can be deleted using `SettingsAction::PolicyRemove`:

**File**: `programs/squads_smart_account_program/src/state/settings_transaction.rs`

```rust
SettingsAction::PolicyRemove {
    policy: Pubkey // The policy account to remove
}
```

The deletion process verifies policy ownership and closes the account.

## Policy Expiration

**File**: `programs/squads_smart_account_program/src/state/policies/policy_core/policy.rs`

```rust
pub enum PolicyExpiration {
    /// Policy expires at a specific timestamp
    Timestamp(i64),
    /// Policy expires when the core settings hash mismatches the stored hash
    SettingsState([u8; 32]),
}
```

- **Timestamp**: Unix timestamp-based expiration
- **SettingsState**: Hash-based expiration when parent smart account changes

## Policy Types

### 1. SpendingLimitPolicy

**File**: `programs/squads_smart_account_program/src/state/policies/implementations/spending_limit_policy.rs`

Allows token and SOL transfers within predefined parameters:
- Source account scoping (specific vault index)
- Destination allowlist
- Time-based limits with automatic resets
- Usage tracking

### 2. SettingsChangePolicy

**File**: `programs/squads_smart_account_program/src/state/policies/implementations/settings_change.rs`

Pre-approved modifications to smart account settings:
- AddSigner with optional constraints
- RemoveSigner with optional constraints
- ChangeThreshold
- ChangeTimeLock

### 3. ProgramInteractionPolicy

**File**: `programs/squads_smart_account_program/src/state/policies/implementations/program_interaction.rs`

Constrained execution of arbitrary program instructions:
- Program ID filtering
- Account constraints
- Data constraints (discriminators, amounts, etc.)
- Optional spending limits integration

Data constraint system supports:
```rust
pub enum DataValue {
    U8(u8), U16Le(u16), U32Le(u32), U64Le(u64), U128Le(u128),
    U8Slice(Vec<u8>), // For discriminators
}

pub enum DataOperator {
    Equals, NotEquals, GreaterThan, GreaterThanOrEqualTo,
    LessThan, LessThanOrEqualTo,
}
```

### 4. InternalFundTransferPolicy

**File**: `programs/squads_smart_account_program/src/state/policies/implementations/internal_fund_transfer.rs`

Transfer funds between smart account vaults:
- Bitmask representation of allowed source/destination indices
- Mint allowlist (optional)

## Transaction Execution Modes

### Consensus-Based (Asynchronous)
Full governance process: Policy Transaction → Proposal → Voting → Time Lock → Execution

### Synchronous Execution
Direct execution with all required signatures present. Requires time lock = 0 and all signers present.

### Policy-Specific Execution
Direct policy execution bypassing consensus for pre-approved actions like spending limits.

## Transaction Cleanup

### Close Empty Policy Transaction

**File**: `programs/squads_smart_account_program/src/instructions/transaction_close.rs`

Cleans up transactions and proposals associated with deleted policy accounts. Validates that the policy account is empty and owned by the system program, then allows unconditional cleanup since the policy address can never be reused.

## File Locations

**Core Policy System**:
- `programs/squads_smart_account_program/src/state/policies/policy_core/`
- `programs/squads_smart_account_program/src/interface/consensus.rs`

**Policy Implementations**:
- `programs/squads_smart_account_program/src/state/policies/implementations/`

**Settings Integration**:
- `programs/squads_smart_account_program/src/state/settings.rs`
- `programs/squads_smart_account_program/src/state/settings_transaction.rs`

**Instructions**:
- `programs/squads_smart_account_program/src/instructions/transaction_close.rs`
- `programs/squads_smart_account_program/src/lib.rs` (entry points)

**Tests**:
- `tests/suites/instructions/policy*.ts`