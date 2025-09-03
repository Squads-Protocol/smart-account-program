# Squads Smart Account Policy System - Complete Guide

## Table of Contents
1. [Overview](#overview)
2. [What Are Policies?](#what-are-policies)
3. [Key Architectural Changes](#key-architectural-changes)
4. [Policy Architecture Deep Dive](#policy-architecture-deep-dive)
5. [Policy Types](#policy-types)
6. [Transaction Execution Modes](#transaction-execution-modes)
7. [Implementation Tutorial](#implementation-tutorial)
8. [API Reference](#api-reference)
9. [Migration Guide](#migration-guide)
10. [Testing & Development](#testing--development)

## Overview

This branch implements a **unified policy framework** for the Squads Smart Account Program. Policies are pre-approved, parameterized governance mechanisms that allow specific actions to be executed with reduced overhead compared to full smart account governance.

### Key Benefits
- **Reduced Transaction Costs**: Pre-approved actions bypass full governance overhead
- **Type Safety**: Each policy type has compile-time guarantees about its behavior  
- **Unified Interface**: Both settings and policies use the same consensus mechanism
- **Extensibility**: New policy types can be added by implementing the `PolicyTrait`
- **Security**: Policies are constrained to their specific use cases with built-in validation

## What Are Policies?

Think of policies as **"mini smart accounts"** with their own:
- Signers and permissions
- Approval thresholds  
- Time locks
- Specific, constrained functionality

### Policy vs Full Governance Comparison

| Aspect | Full Governance (Settings) | Policy |
|--------|---------------------------|---------|
| **Flexibility** | Execute arbitrary instructions | Limited to policy-specific actions |
| **Setup Cost** | High (full proposal/voting flow) | Lower (pre-approved parameters) |
| **Security Model** | Maximum flexibility, maximum risk | Constrained actions, reduced risk |
| **Use Cases** | Major changes, arbitrary operations | Routine operations, spending limits |

## Key Architectural Changes

### 1. Consensus Trait Unification

The biggest change is the introduction of a `Consensus` trait that both `Settings` and `Policy` accounts implement:

```rust
pub trait Consensus {
    // Core consensus methods
    fn signers(&self) -> &[SmartAccountSigner];
    fn threshold(&self) -> u16;
    fn time_lock(&self) -> u32;
    fn transaction_index(&self) -> u64;
    
    // Permission checking
    fn is_signer(&self, signer_pubkey: Pubkey) -> Option<usize>;
    fn signer_has_permission(&self, signer_pubkey: Pubkey, permission: Permission) -> bool;
    
    // Validation
    fn invariant(&self) -> Result<()>;
}
```

This allows both account types to be used interchangeably:

```rust
pub enum ConsensusAccount {
    Settings(Settings),  // Full governance
    Policy(Policy),      // Policy-specific governance
}
```

### 2. Transaction Payload Types

Transactions now support two distinct payload types:

```rust
pub enum CreateTransactionArgs {
    TransactionPayload {
        account_index: u8,
        ephemeral_signers: u8,
        transaction_message: Vec<u8>,  // Arbitrary Solana instructions
        memo: Option<String>,
    },
    PolicyPayload {
        payload: PolicyPayload,        // Policy-specific actions
    },
}
```

### 3. Unified Instruction Flow

All instructions now accept `InterfaceAccount<'info, ConsensusAccount>`, enabling the same instruction to work with both settings and policies.

## Policy Architecture Deep Dive

### Policy Account Structure

```rust
#[account]
pub struct Policy {
    // Consensus fields (shared with Settings)
    pub settings: Pubkey,                    // Parent smart account
    pub seed: u64,                          // Unique policy identifier
    pub transaction_index: u64,             // Latest transaction number
    pub stale_transaction_index: u64,       // Staleness boundary
    pub signers: Vec<SmartAccountSigner>,   // Members with permissions
    pub threshold: u16,                     // Approval threshold
    pub time_lock: u32,                     // Execution delay (seconds)
    
    // Policy-specific fields
    pub policy_state: PolicyState,          // Type-specific configuration
    pub start: i64,                         // Activation timestamp
    pub expiration: Option<PolicyExpiration>, // Expiration rules
    pub bump: u8,                           // PDA bump
}
```

### Policy State Enum

```rust
pub enum PolicyState {
    InternalFundTransfer(InternalFundTransferPolicy),
    SpendingLimit(SpendingLimitPolicy),
    SettingsChange(SettingsChangePolicy),
    ProgramInteraction(ProgramInteractionPolicy),
}
```

### Policy Trait System

All policies implement the `PolicyTrait` for type-safe execution:

```rust
pub trait PolicyTrait {
    type PolicyState;
    type CreationPayload: PolicyPayloadConversionTrait<PolicyState = Self::PolicyState> + PolicySizeTrait;
    type UsagePayload;
    type ExecutionArgs;

    // Validate policy state
    fn invariant(&self) -> Result<()>;
    
    // Validate execution payload
    fn validate_payload(&self, context: PolicyExecutionContext, payload: &Self::UsagePayload) -> Result<()>;
    
    // Execute the policy action
    fn execute_payload<'info>(&mut self, args: Self::ExecutionArgs, payload: &Self::UsagePayload, accounts: &'info [AccountInfo<'info>]) -> Result<()>;
}
```

## Policy Types

### 1. SpendingLimitPolicy

**Purpose**: Token and SOL transfers within predefined limits

**Key Features**:
- Time-based limits with automatic resets (daily, weekly, monthly)
- Usage tracking and remaining amount calculation
- Source account scoping (specific vault index)
- Destination allowlist
- Supports both native SOL and SPL tokens

**Creation Payload**:
```rust
pub struct SpendingLimitPolicyCreationPayload {
    pub mint: Pubkey,                    // Token mint (Pubkey::default() for SOL)
    pub source_account_index: u8,        // Which vault can spend
    pub time_constraints: TimeConstraints, // When spending is allowed
    pub quantity_constraints: QuantityConstraints, // How much can be spent
    pub usage_state: Option<UsageState>, // Current usage state
    pub destinations: Vec<Pubkey>,       // Allowed destinations
}
```

**Execution Payload**:
```rust
pub struct SpendingLimitPayload {
    pub amount: u64,
    pub destination: Pubkey,
    pub decimals: u8,
}
```

**Example Use Case**: Allow marketing team to spend up to 1000 USDC per month to specific vendor addresses.

### 2. InternalFundTransferPolicy

**Purpose**: Transfer funds between smart account vaults

**Key Features**:
- Bitmask representation of allowed source/destination indices (space efficient)
- Optional mint allowlist (empty = all mints allowed)
- Prevents transfers between the same vault

**Creation Payload**:
```rust
pub struct InternalFundTransferPolicyCreationPayload {
    pub source_account_indices: Vec<u8>,      // Which vaults can be sources
    pub destination_account_indices: Vec<u8>, // Which vaults can be destinations
    pub allowed_mints: Vec<Pubkey>,           // Optional mint restrictions
}
```

**Execution Payload**:
```rust
pub struct InternalFundTransferPayload {
    pub source_index: u8,
    pub destination_index: u8,
    pub mint: Pubkey,
    pub decimals: u8,
    pub amount: u64,
}
```

**Example Use Case**: Allow treasury operations to move funds between operational and reserve vaults.

### 3. SettingsChangePolicy

**Purpose**: Pre-approved modifications to smart account settings

**Allowed Actions**:
- Add/remove signers (with optional constraints)
- Change threshold
- Change timelock

**Creation Payload**:
```rust
pub struct SettingsChangePolicyCreationPayload {
    pub actions: Vec<AllowedSettingsChange>,
}

pub enum AllowedSettingsChange {
    AddSigner { 
        new_signer: Option<Pubkey>,           // None = any signer allowed
        new_signer_permissions: Option<Permissions>, // None = any permissions
    },
    RemoveSigner { 
        old_signer: Option<Pubkey>,           // None = any signer can be removed
    },
    ChangeThreshold,
    ChangeTimeLock { 
        new_time_lock: Option<u32>,           // None = any timelock allowed
    },
}
```

**Example Use Case**: Allow HR to add new employees as signers with specific permission sets.

### 4. ProgramInteractionPolicy

**Purpose**: Constrained execution of arbitrary program instructions

**Constraint System**:
- Program ID filtering
- Account constraints
- Data constraints (discriminators, amounts, etc.)
- Optional spending limits integration

**Data Constraint System**:
```rust
pub enum DataValue {
    U8(u8), U16Le(u16), U32Le(u32), U64Le(u64), U128Le(u128),
    U8Slice(Vec<u8>), // For discriminators
}

pub enum DataOperator {
    Equals, NotEquals, GreaterThan, GreaterThanOrEqualTo,
    LessThan, LessThanOrEqualTo,
}

pub struct InstructionConstraint {
    pub program_id: Pubkey,
    pub data_constraints: Vec<DataConstraint>,
    pub account_constraints: Vec<AccountConstraint>,
}
```

**Example Use Case**: Allow DeFi operations team to interact with specific protocols with amount limits.

## Transaction Execution Modes

### 1. Asynchronous Execution (Traditional)
**Flow**: Create Transaction → Create Proposal → Vote → Execute
- Full governance process
- Supports time locks
- Requires threshold approvals

### 2. Synchronous Execution  
**Flow**: Direct execution with all signatures present
- **Requirements**: `time_lock = 0` and all required signers present
- Bypasses proposal/voting overhead
- Immediate execution

### 3. Policy-Specific Execution
**Flow**: Validate against policy constraints → Execute policy logic
- Type-safe execution
- Built-in validation
- Constraint enforcement

## Implementation Tutorial

### Step 1: Create a Policy

Policies are created through the full smart account governance process using a settings transaction:

```typescript
// 1. Create a settings transaction with PolicyCreate action
const createPolicyTx = await createSettingsTransaction({
  smartAccount: smartAccountPubkey,
  action: {
    policyCreate: {
      policySeed: new BN(1),
      policySigners: [
        { key: signer1.publicKey, permissions: { initiate: true, vote: true, execute: true } },
        { key: signer2.publicKey, permissions: { initiate: true, vote: true, execute: false } },
      ],
      policyThreshold: 2,
      policyTimeLock: 0, // Immediate execution
      policyCreationPayload: {
        spendingLimit: {
          mint: USDC_MINT,
          sourceAccountIndex: 0,
          timeConstraints: {
            start: Math.floor(Date.now() / 1000),
            period: { month: {} },
            accumulateUnused: false,
          },
          quantityConstraints: {
            maxPerPeriod: new BN(1000 * 1e6), // 1000 USDC
          },
          destinations: [vendorAddress],
        }
      },
      policyStart: Math.floor(Date.now() / 1000),
      policyExpiration: {
        timestamp: Math.floor(Date.now() / 1000) + (365 * 24 * 60 * 60), // 1 year
      },
    }
  }
});

// 2. Go through proposal/voting process for the settings transaction
// 3. Execute the settings transaction to create the policy
```

### Step 2: Use a Policy

Once created, policies can be used directly:

```typescript
// For spending limit policy
const usePolicyTx = await createTransaction({
  consensusAccount: policyPubkey, // Note: using policy as consensus account
  args: {
    policyPayload: {
      spendingLimit: {
        amount: new BN(500 * 1e6), // 500 USDC
        destination: vendorAddress,
        decimals: 6,
      }
    }
  }
});

// Can execute synchronously if timelock = 0 and all signers present
await executeSyncTransaction(usePolicyTx, allRequiredSigners);
```

### Step 3: Policy Expiration

Policies can expire in two ways:

```rust
pub enum PolicyExpiration {
    Timestamp(i64),           // Expires at specific Unix timestamp
    SettingsState([u8; 32]),  // Expires when parent smart account changes
}
```

**Timestamp Expiration**: Policy becomes inactive after the specified time.

**Settings State Expiration**: Policy becomes inactive when the parent smart account's core settings (signers, threshold, timelock) change. This ensures policies don't become stale when governance structure changes.

## API Reference

### Core Instructions

#### Policy Creation
- **Instruction**: `create_settings_transaction` with `PolicyCreate` action
- **Purpose**: Create a new policy through full governance
- **Accounts**: Settings account, transaction account, proposal account

#### Policy Execution (Async)
- **Instruction**: `create_transaction` → `create_proposal` → `execute_transaction`
- **Purpose**: Execute policy through traditional governance flow
- **Accounts**: Policy account (as consensus), transaction account, proposal account

#### Policy Execution (Sync)
- **Instruction**: `execute_transaction_sync_v2`
- **Purpose**: Direct policy execution with all signatures
- **Requirements**: `time_lock = 0`, all required signers present

#### Policy Cleanup
- **Instruction**: `close_empty_policy_transaction`
- **Purpose**: Clean up transactions for deleted policies
- **Use Case**: When a policy is removed, clean up any pending transactions

### Policy-Specific Instructions

Each policy type has its own execution logic but uses the same instruction interface:

```rust
// All policies use the same instruction signature
pub fn execute_transaction_sync_v2<'info>(
    ctx: Context<'_, '_, 'info, 'info, SyncTransaction<'info>>,
    args: SyncTransactionArgs,
) -> Result<()>

// But with different payload types
pub enum SyncPayload {
    Transaction(TransactionPayload),
    Policy(PolicyPayload),
}

pub enum PolicyPayload {
    InternalFundTransfer(InternalFundTransferPayload),
    SpendingLimit(SpendingLimitPayload),
    SettingsChange(SettingsChangePayload),
    ProgramInteraction(ProgramInteractionPayload),
}
```

### Account Derivations

#### Policy Account PDA
```rust
// Policy accounts are derived from parent settings + seed
let (policy_pubkey, bump) = Pubkey::find_program_address(
    &[
        SEED_PREFIX,
        settings_pubkey.as_ref(),
        SEED_POLICY,
        &policy_seed.to_le_bytes(),
    ],
    &program_id,
);
```

#### Smart Account Vault PDA
```rust
// Vault accounts (used by policies for transfers)
let (vault_pubkey, bump) = Pubkey::find_program_address(
    &[
        SEED_PREFIX,
        settings_pubkey.as_ref(),
        SEED_SMART_ACCOUNT,
        &vault_index.to_le_bytes(),
    ],
    &program_id,
);
```

## Migration Guide

### From Legacy Spending Limits

The old `use_spending_limit` instruction is deprecated in favor of spending limit policies:

**Before**:
```rust
// Old approach - spending limits were part of settings
pub fn use_spending_limit(
    ctx: Context<UseSpendingLimit>,
    args: UseSpendingLimitArgs,
) -> Result<()>
```

**After**:
```rust
// New approach - spending limits are policies
pub fn execute_transaction_sync_v2<'info>(
    ctx: Context<'_, '_, 'info, 'info, SyncTransaction<'info>>,
    args: SyncTransactionArgs, // Contains PolicyPayload::SpendingLimit
) -> Result<()>
```

**Migration Steps**:
1. Create spending limit policies to replace old spending limits
2. Update client code to use policy execution instead of `use_spending_limit`
3. Remove old spending limit configurations from settings

### From Legacy Sync Execution

The old `execute_transaction_sync` is deprecated:

**Before**:
```rust
pub fn execute_transaction_sync(
    ctx: Context<LegacySyncTransaction>,
    args: LegacySyncTransactionArgs,
) -> Result<()>
```

**After**:
```rust
pub fn execute_transaction_sync_v2<'info>(
    ctx: Context<'_, '_, 'info, 'info, SyncTransaction<'info>>,
    args: SyncTransactionArgs,
) -> Result<()>
```

## Testing & Development

### Running Tests

```bash
# Run all tests
anchor test

# Run policy-specific tests
anchor test -- --grep "policy"

# Run specific policy type tests
anchor test -- --grep "spending_limit"
anchor test -- --grep "internal_fund_transfer"
```

### Test Structure

Policy tests are organized by type:
- `tests/suites/instructions/policy_spending_limit.ts`
- `tests/suites/instructions/policy_internal_fund_transfer.ts`
- `tests/suites/instructions/policy_settings_change.ts`
- `tests/suites/instructions/policy_program_interaction.ts`

### Development Environment

```bash
# Build the program
anchor build

# Deploy to localnet
anchor deploy

# Generate TypeScript client
anchor run generate-client
```

### SDK Usage

The TypeScript SDK provides type-safe policy creation and execution:

```typescript
import { 
  createSettingsTransaction,
  createTransaction,
  executeSyncTransaction,
  PolicyCreationPayload,
  PolicyPayload 
} from '@sqds/smart-account';

// Create policy
const policyTx = await createSettingsTransaction({
  action: {
    policyCreate: {
      // ... policy configuration
    }
  }
});

// Use policy
const useTx = await createTransaction({
  consensusAccount: policyPubkey,
  args: {
    policyPayload: {
      spendingLimit: {
        amount: new BN(1000),
        destination: recipientAddress,
        decimals: 6,
      }
    }
  }
});
```

## Debugging & Troubleshooting

### Common Issues

1. **Policy Not Active**: Check `start` timestamp and `expiration` conditions
2. **Insufficient Permissions**: Verify signer has required permissions on policy
3. **Validation Failures**: Check payload matches policy constraints
4. **Stale Transactions**: Policy changes invalidate pending transactions

### Event Logging

The system emits comprehensive events for debugging:

```rust
// Policy creation events
PolicyEvent { event_type: Create, policy: Policy, ... }

// Policy execution events  
PolicyEvent { event_type: UpdateDuringExecution, policy: Policy, ... }

// Transaction events
TransactionEvent { event_type: Execute, transaction_content: PolicyPayload, ... }
```

### Error Codes

Key policy-related errors:
- `ConsensusAccountNotPolicy`: Trying to use settings account as policy
- `SpendingLimitPolicyInvariantAmountExceeded`: Spending limit exceeded
- `InternalFundTransferPolicyInvariantSourceAccountIndexNotAllowed`: Invalid source vault
- `SettingsChangeActionMismatch`: Policy action doesn't match allowed actions

---

## Conclusion

The policy system represents a major evolution in smart account governance, providing:

1. **Efficiency**: Pre-approved actions with reduced overhead
2. **Safety**: Type-safe, constrained execution
3. **Flexibility**: Multiple execution modes (sync/async)
4. **Extensibility**: Easy to add new policy types

This unified framework enables sophisticated governance patterns while maintaining the security and reliability of the Squads Smart Account system.

For more examples and advanced usage patterns, see the test suites in `tests/suites/instructions/policy_*.ts`.
