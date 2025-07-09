# Squads Smart Account Program Summary

## Core Architecture

**Smart Account** = Multi-signature wallet with configurable governance and spending controls

## Three Transaction Execution Modes

### 1. Consensus-Based Transactions (Full Governance)
- **Purpose**: Any blockchain operation requiring multi-sig approval
- **Flow**: Create Transaction → Create Proposal → Vote → Time Lock → Execute
- **Components**:
  - **Settings**: Signers, threshold, time lock configuration
  - **Proposals**: Voting records (approved/rejected/cancelled)
  - **Permissions**: Initiate, Vote, Execute roles
  - **Stale Protection**: Settings changes invalidate old proposals

### 2. Synchronous Transactions (Immediate Execution)
- **Purpose**: Same as consensus transactions but executed immediately
- **Flow**: Single instruction with all required signatures
- **Requirements**:
  - Time lock must be 0 (no deliberation period)
  - All required signers present simultaneously
  - Combined permissions must include Initiate + Vote + Execute
  - Meet threshold requirements
- **Benefits**: Gas efficient, atomic execution, reduced latency
- **Limitations**: No time lock support, requires coordination of all signers

### 3. Spending Limits (Pre-Authorized Bypass)
- **Purpose**: Token transfers within pre-approved parameters
- **Flow**: Create Spending Limit → Use Spending Limit (direct execution)
- **Parameters**:
  - Amount & token type
  - Reset periods (OneTime, Daily, Weekly, Monthly)
  - Authorized signers
  - Destination allowlist
  - Expiration date

## Key Security Mechanisms

### Stale Transaction Protection
- `transaction_index`: Latest transaction number
- `stale_transaction_index`: Staleness boundary
- When signers/threshold/timelock change → all pending transactions become stale
- Settings transactions cannot execute if stale (security critical)
- Regular transactions can execute if stale (but approved before staleness)

### Permission System
- **Initiate**: Create transactions/proposals
- **Vote**: Approve/reject/cancel proposals  
- **Execute**: Execute approved transactions

### Time Locks
- Mandatory delay between approval and execution
- Prevents immediate execution attacks
- Can be removed by settings authority

## Account Types
- **Autonomous**: Self-governed via proposals
- **Controlled**: Has settings authority that can bypass consensus for configuration

## Transaction Mode Comparison
- **Consensus Transactions**: Any operation, full governance with voting + time lock
- **Synchronous Transactions**: Any operation, immediate execution with all signatures
- **Spending Limits**: Token transfers only, pre-authorized bypass
- All three are separate flows with no overlap or conflict

This creates a flexible system balancing security (consensus) with efficiency (spending limits) for treasury management.

## Policy Framework (Proposed Extension)

### Overview
Spending limits are being evolved into a generalized **Policy Framework** where spending limits become just one type of policy among many. This creates a unified governance system for all types of smart account policies.

### Policy Account Structure (Conceptual)
```rust
pub struct Policy {
    // Core governance (unified across all policy types)
    pub smart_account: Pubkey,           // Parent smart account
    pub transaction_index: u64,          // Stale transaction protection
    pub stale_transaction_index: u64,    // Staleness boundary
    pub signers: Vec<PolicySigner>,      // Members with permissions
    pub threshold: u16,                  // Approval threshold
    pub time_lock: u32,                  // Delay before execution
    
    // Policy-specific configuration
    pub policy_type: PolicyType,         // Enum defining policy type
    pub policy_data: Vec<u8>,           // Serialized policy-specific parameters
    pub vault_scopes: Vec<u8>,          // Which vaults this policy applies to
    
    // Metadata
    pub bump: u8,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct PolicySigner {
    pub key: Pubkey,
    pub permissions: Permissions,        // Initiate, Vote, Execute
}

pub enum PolicyType {
    SpendingLimit,                       // Current spending limits (token transfers)
    ProgramInteraction,                  // Arbitrary program calls with constraints
    // Future types: MultiSigOverride, ComplianceRule, etc.
}
```

### Policy Requirements (From GRID-600)
1. **Every signer on every policy has permissions** - PolicySigner structure with Initiate/Vote/Execute
2. **Every policy has thresholds** - Unified threshold field across all policy types
3. **Every policy can specify which vaults it's active on** - vault_scopes field
4. **Every policy offers alternate form of consensus** - Alternative to full smart account governance
5. **Overflowing amounts per spending limit duration** - Enhanced spending limit features
6. **Start date for spending limits** - Time-based activation
7. **Declarable remaining amount in Spending Limits** - Better limit tracking

### Policy Types

#### SpendingLimit Policy (Current Implementation → Policy Type)
- **Purpose**: Token transfers within pre-approved parameters
- **Parameters**: Amount, token, period, destinations, expiration
- **Execution**: Direct bypass of consensus when conditions met

#### ProgramInteraction Policy (Proposed)
- **Purpose**: Allow specific program calls with granular constraints
- **Constraint Types**:
  - **Instruction Data**: Bytes 0-8 (discriminator) must equal specific values
  - **Account Constraints**: Specific accounts or accounts owned by certain programs
  - **Parameter Validation**: Validate specific instruction data fields

**Example Policy Configurations**:
```rust
pub struct ProgramInteractionConstraints {
    pub program_id: Pubkey,
    pub allowed_discriminators: Vec<[u8; 8]>,     // Instruction discriminators allowed
    pub account_constraints: Vec<AccountConstraint>,
    pub data_constraints: Vec<DataConstraint>,    // Validate specific data fields
}

pub enum AccountConstraint {
    MustBe(Pubkey),                               // Account must be specific pubkey
    MustBeOwnedBy(Pubkey),                        // Account must be owned by program
    MustBeDerivedFrom { program: Pubkey, seeds: Vec<Vec<u8>> }, // PDA constraints
}

pub enum DataConstraint {
    BytesAtOffset { offset: usize, bytes: Vec<u8> }, // Specific bytes at offset
    U64AtOffset { offset: usize, max_value: u64 },   // Numeric constraints
}
```

**Use Cases**:
- "Allow Jupiter swaps but only for amounts < 1000 USDC"
- "Allow Serum DEX orders but only on specific markets"
- "Allow staking but only to approved validators"
- **Execution**: Validate instruction matches all policy constraints before execution

### Consensus Trait/Interface (Core Design Concept)
Since policy accounts and smart account settings share so much structure (signers, thresholds, permissions, stale transaction protection), the plan is to create a **unified consensus trait/interface** that allows execution on both:

```rust
pub trait Consensus {
    fn signers(&self) -> &[ConsensusSigner];
    fn threshold(&self) -> u16;
    fn time_lock(&self) -> u32;
    fn transaction_index(&self) -> u64;
    fn stale_transaction_index(&self) -> u64;
    fn validate_execution(&self, signers: &[Pubkey]) -> Result<()>;
    // etc.
}

impl Consensus for Settings { /* smart account implementation */ }
impl Consensus for Policy { /* policy account implementation */ }
```

This means you can execute transactions/proposals on both smart account settings AND individual policy accounts using the same interface and validation logic.

### Benefits
1. **Unified Governance**: All policies use same stale transaction protection, permissions, consensus
2. **Extensible**: New policy types can be added without changing core infrastructure
3. **Consistent UX**: All policies work the same way from governance perspective
4. **Vault Scoping**: Each policy can specify which smart account indices it applies to
5. **Alternative Consensus**: Policies provide lighter-weight governance than full proposals
6. **Code Reuse**: Same consensus logic works for both smart accounts and policies

### Swig Wallet Deep Analysis (Anagram)
**Repository**: https://github.com/anagrambuild/swig-wallet

After analyzing the source code, Swig implements a **sophisticated structured authorization system**, not a general-purpose VM:

#### Core Architecture
- **Role-Based Access Control**: Each wallet has multiple roles, each with specific authorities and actions
- **Action-Based Permissions**: Predefined action types (TokenLimit, SolLimit, ProgramScope, StakeLimit, etc.)
- **Multi-Authority Support**: ED25519, SECP256k1, SECP256r1 cryptographic authorities
- **Session Management**: Time-bounded temporary permissions with slot-based expiration

#### Key Policy Components (from state-x/)

**Role Structure**:
```rust
// Each role contains:
- Authority type + data
- Set of actions/permissions
- Unique role ID
- Position/boundary markers
```

**Action Types**:
- **TokenLimit**: Per-mint spending limits with `current_amount` tracking
- **SolLimit**: Native SOL spending limits  
- **ProgramScope**: External program interaction policies with 3 scope types:
  - Basic (unrestricted)
  - Fixed Limit (total amount cap)
  - Recurring Limit (time-windowed amounts)
- **StakeLimit**: Staking operation constraints
- **All**: Unrestricted access

#### Policy Enforcement Mechanism
1. **Pre-execution validation**: Check role permissions and limits
2. **Real-time tracking**: Update `current_amount` during execution
3. **Account snapshots**: Validate post-execution state changes
4. **Automatic resets**: Time-based limit resets for recurring policies

#### Key Insights for Our Framework
1. **Structured vs VM**: They use predefined action enums, not arbitrary logic
2. **Real-time enforcement**: Validation happens during execution, not pre-authorization
3. **Granular tracking**: Per-mint, per-program, per-operation limits
4. **Session model**: Temporary permissions complement persistent roles
5. **Account snapshots**: Post-execution validation ensures policy compliance

#### Arbitrary Instruction Execution Analysis

**Program Scope System** (from `state-x/src/action/program_scope.rs`):
- **3 Scope Types**:
  - **Basic**: Unrestricted program interaction
  - **Fixed Limit**: Total amount cap across all interactions  
  - **Recurring Limit**: Time-windowed (slot-based) amount limits with automatic resets
- **Balance Tracking**: Reads specific byte ranges in account data to track balances/amounts
- **Automatic Resets**: Time-based limit resets using Solana slot numbers

**Program Permission System** (from `state-x/src/action/program.rs`):
- **Explicit Program Allowlist**: 32-byte program ID matching for allowed interactions
- **Repeatable Permissions**: Multiple program interactions allowed per role
- **Strict Validation**: Exact byte-level matching of program identifiers

**Instruction Execution Constraints** (from `program/src/actions/sign_v1.rs`):
- **Pre-Execution**: Role authentication, program scope check, account classification, balance validation
- **Execution Control**: Account snapshots, controlled execution, post-execution validation
- **Balance Tracking**: Updates `current_amount` for limit enforcement

**What Swig Does NOT Constrain**:
- **No Instruction Discriminator Filtering**: Doesn't restrict specific instruction types within allowed programs
- **No Instruction Data Validation**: Doesn't parse or constrain instruction data content  
- **No Account Constraint Logic**: Doesn't enforce "account must be owned by X" style rules

**Swig's Execution Model**:
```rust
// Simple program allowlist + balance limits
ProgramScope {
    program_id: Pubkey,              // Which program can be called
    scope_type: Basic/Fixed/Recurring, // How much interaction allowed
    balance_range: (offset, length),    // What balance field to track
    current_amount: u64,             // Real-time usage tracking
}
```

**Key Characteristics**:
- **Program-level permissions** rather than instruction-level granularity
- **"How much" focus** - tracks amounts/balances rather than constraining specific operations
- **Real-time enforcement** during execution with account snapshots
- **Balance tracking within account data** for DeFi interactions

**Relevance**: Swig validates our policy account approach but with more sophisticated real-time enforcement and granular action tracking. Their "policy engine" is actually a structured permission system with predefined types.

### Migration Path
- Current spending limits → SpendingLimit policy type
- Existing spending limit accounts can be migrated or left as-is
- New policies created using unified Policy account structure

## Key File Locations
- Core state: `programs/squads_smart_account_program/src/state/`
- Instructions: `programs/squads_smart_account_program/src/instructions/`
- Tests: `tests/suites/examples/`
- SDK: `sdk/smart-account/`