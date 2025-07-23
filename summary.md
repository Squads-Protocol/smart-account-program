# Policy Account Architecture Design Problem

## Context

The Squads Smart Account Program is implementing a unified policy framework where different types of policies (spending limits, program interactions, internal fund transfers, etc.) can be executed through a common consensus mechanism. The goal is to make policy execution as idiomatic and extensible as possible.

## Current Architecture

### Existing Structure
The current implementation uses a single `Policy` account with an enum-based approach:

```rust
#[account]
pub struct Policy {
    // Common consensus fields (90% shared across all policy types)
    pub settings: Pubkey,
    pub transaction_index: u64,
    pub stale_transaction_index: u64,
    pub signers: Vec<PolicySigner>,
    pub threshold: u16,
    pub time_lock: u32,
    pub vault_scopes: Vec<u8>,
    
    // Policy type identification
    pub policy_type: PolicyType,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum PolicyType {
    InternalFundTransfer(InternalFundTransferPolicy),
    SpendingLimit(SpendingLimitPolicy),
    // Future policy types...
}
```

### Current Execution Flow
1. **Transaction Creation**: `CreateTransactionArgs` enum handles both `TransactionPayload` (for settings) and `PolicyPayload` (for policies)
2. **Transaction Execution**: Pattern matching on `transaction.payload` type in `transaction_execute.rs`
3. **Policy Dispatch**: Manual pattern matching on policy type with incomplete execution logic

### ConsensusAccount Trait
All consensus accounts (Settings and Policies) implement a unified `Consensus` trait that provides:
- Signer management (`signers()`, `is_signer()`, `signer_has_permission()`)
- Transaction indexing (`transaction_index()`, `set_transaction_index()`)
- Threshold and timelock management
- Stale transaction protection

Instructions use `InterfaceAccount<'info, ConsensusAccount>` to work with any consensus account type uniformly.

## The Problem

### Current Issues
1. **Inconsistent Execution Pattern**: Each policy type requires manual pattern matching in execution logic
2. **Unused Abstractions**: `PolicyExecution` trait exists but isn't properly utilized
3. **Ad-hoc Dispatch**: `PolicyPayload::execute()` manually dispatches to policy implementations
4. **Maintenance Burden**: Adding new policy types requires changes to multiple match statements
5. **State Management Complexity**: Policy-specific data is embedded in enums, making field access verbose

### Core Dilemma
The fundamental tension is between:
- **Type Safety & Performance**: Direct field access (`policy.field = value`) with compile-time guarantees
- **Unified Interface**: Single account type that works with `InterfaceAccount<'info, ConsensusAccount>`
- **Code Reuse**: Avoiding duplication of the 90% shared consensus fields across policy types

## Explored Solutions

### Option 1: Composition with Serialized Data
```rust
#[account]
pub struct Policy {
    // Common consensus fields
    pub settings: Pubkey,
    pub transaction_index: u64,
    // ... other consensus fields
    
    pub policy_discriminator: u8,
    pub policy_data: Vec<u8>,  // Serialized policy-specific data
}
```

**Pros**: Zero duplication, unified interface
**Cons**: Serialization overhead for every state change, loss of type safety

### Option 2: Macro-Generated Separate Types
```rust
macro_rules! policy_account {
    ($name:ident { $(pub $field:ident: $type:ty,)* }) => {
        #[account]
        pub struct $name {
            // Auto-generated consensus fields
            pub settings: Pubkey,
            pub transaction_index: u64,
            // ... 
            
            // Policy-specific fields
            $(pub $field: $type,)*
        }
    };
}
```

**Pros**: Zero runtime overhead, full type safety, direct field access
**Cons**: Breaks `InterfaceAccount<'info, ConsensusAccount>` - multiple distinct types can't be treated uniformly

### Option 3: Composition with Embedded Struct
```rust
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct PolicyConsensus {
    pub settings: Pubkey,
    pub transaction_index: u64,
    // ... consensus fields
}

#[account]
pub struct InternalFundTransferPolicy {
    pub consensus: PolicyConsensus,
    pub source_account_indices: Vec<u8>,
    // ... policy-specific fields
}
```

**Pros**: Direct field access, type safety, minimal duplication
**Cons**: Still breaks unified interface, requires trait delegation boilerplate

### Option 4: Enum for Interface with Separate Types
```rust
pub enum ConsensusAccountData {
    Settings(Settings),
    InternalFundTransferPolicy(InternalFundTransferPolicy),
    SpendingLimitPolicy(SpendingLimitPolicy),
}

impl ConsensusAccount for ConsensusAccountData {
    fn signers(&self) -> &[PolicySigner] {
        match self {
            ConsensusAccountData::Settings(s) => &s.signers,
            ConsensusAccountData::InternalFundTransferPolicy(p) => &p.consensus.signers,
            // ... pattern match for each type
        }
    }
}
```

**Pros**: Maintains unified interface, type-safe field access
**Cons**: Method delegation boilerplate for every consensus method on every policy type

## Core Architectural Constraints

### Anchor Framework Limitations
- `InterfaceAccount<'info, T>` requires a single trait object type
- Account types must be known at compile time for space allocation
- Serialization format is fixed per account type

### Performance Requirements
- State changes should be direct field assignments, not serialize/deserialize cycles
- Consensus operations (voting, execution) happen frequently and must be efficient

### Extensibility Goals
- Adding new policy types should require minimal changes to existing code
- Policy execution should be pluggable and uniform
- Each policy should own its specific logic and data

## Key Questions for Resolution

1. **Interface vs Type Safety**: Is it acceptable to break the unified `InterfaceAccount` interface to gain type safety and performance?

2. **Code Generation**: Are macros an acceptable solution for eliminating boilerplate, or do they introduce too much complexity?

3. **Runtime Dispatch**: Is the performance cost of serialization/deserialization acceptable for the benefits of a unified interface?

4. **Trait Delegation**: Is repetitive trait implementation across policy types an acceptable trade-off for direct field access?

5. **Alternative Architectures**: Are there Rust/Anchor patterns we haven't considered that could solve this trilemma?

## Success Criteria

The ideal solution should provide:
- **Ergonomic state management**: `policy.field = value` level simplicity
- **Type safety**: Compile-time guarantees for policy-specific fields
- **Unified interface**: Works with existing consensus trait and instruction patterns
- **Zero/minimal duplication**: Don't repeat the 90% shared consensus logic
- **Extensibility**: Adding new policies requires only implementing policy-specific logic
- **Performance**: No unnecessary serialization overhead for state changes

## Policy Types Context

### Current Policy Types
- **InternalFundTransfer**: Transfer funds between smart account vaults
- **SpendingLimit**: Token spending limits with time constraints and usage tracking
- **ProgramInteraction**: Constrained program calls with instruction/data validation

### Policy Execution Pattern
All policies follow the pattern:
1. Receive a policy-specific payload (execution parameters)
2. Validate the payload against policy constraints
3. Execute the action using provided `remaining_accounts`
4. Update policy state (usage tracking, etc.)

### Consensus Integration
Policies implement the same consensus mechanism as Settings:
- Multi-signature approval with configurable thresholds
- Time locks between approval and execution
- Stale transaction protection when policy parameters change
- Permission-based access control (Initiate, Vote, Execute)