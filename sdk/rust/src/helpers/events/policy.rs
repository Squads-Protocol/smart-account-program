//! Policy types ported from the program's `state/policies/` subtree.
//!
//! These are **data-only** mirror types — the program-side versions carry
//! invariant checks, size accounting, and `validate_payload` business logic;
//! none of that is meaningful off-chain, so we only port the struct/enum
//! shapes needed for borsh round-trip of event payloads and on-chain account
//! decoding.
//!
//! When the program's `policies/` types change, these need to be kept in
//! sync. The cleanest way is to compare against
//! `programs/squads_smart_account_program/src/state/policies/**` directly.

use borsh::{BorshDeserialize, BorshSerialize};
use solana_address::Address;

use crate::generated::types::{Permissions, SmartAccountSigner};

// ---------------------------------------------------------------------------
// Top-level policy enums
// ---------------------------------------------------------------------------

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum PolicyCreationPayload {
    InternalFundTransfer(InternalFundTransferPolicyCreationPayload),
    SpendingLimit(SpendingLimitPolicyCreationPayload),
    SettingsChange(SettingsChangePolicyCreationPayload),
    ProgramInteraction(ProgramInteractionPolicyCreationPayload),
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum PolicyPayload {
    InternalFundTransfer(InternalFundTransferPayload),
    ProgramInteraction(ProgramInteractionPayload),
    SpendingLimit(SpendingLimitPayload),
    SettingsChange(SettingsChangePayload),
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum PolicyExpiration {
    Timestamp(i64),
    SettingsState([u8; 32]),
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum PolicyExpirationArgs {
    Timestamp(i64),
    SettingsState,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum PolicyState {
    InternalFundTransfer(InternalFundTransferPolicy),
    SpendingLimit(SpendingLimitPolicy),
    SettingsChange(SettingsChangePolicy),
    ProgramInteraction(ProgramInteractionPolicy),
}

/// Mirror of the program's `state/policies/policy_core/policy.rs::Policy`
/// (no discriminator). Used inside `PolicyEvent::policy`.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct Policy {
    pub settings: Address,
    pub seed: u64,
    pub bump: u8,
    pub transaction_index: u64,
    pub stale_transaction_index: u64,
    pub signers: Vec<SmartAccountSigner>,
    pub threshold: u16,
    pub time_lock: u32,
    pub policy_state: PolicyState,
    pub start: i64,
    pub expiration: Option<PolicyExpiration>,
    pub rent_collector: Address,
}

// ---------------------------------------------------------------------------
// Internal fund transfer policy
// ---------------------------------------------------------------------------

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct InternalFundTransferPolicy {
    pub source_account_mask: [u8; 32],
    pub destination_account_mask: [u8; 32],
    pub allowed_mints: Vec<Address>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct InternalFundTransferPayload {
    pub source_index: u8,
    pub destination_index: u8,
    pub mint: Address,
    pub decimals: u8,
    pub amount: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct InternalFundTransferPolicyCreationPayload {
    pub source_account_indices: Vec<u8>,
    pub destination_account_indices: Vec<u8>,
    pub allowed_mints: Vec<Address>,
}

// ---------------------------------------------------------------------------
// Spending-limit policy
// ---------------------------------------------------------------------------

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeriodV2 {
    OneTime,
    Daily,
    Weekly,
    Monthly,
    Custom(i64),
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeConstraints {
    pub start: i64,
    pub expiration: Option<i64>,
    pub period: PeriodV2,
    pub accumulate_unused: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantityConstraints {
    pub max_per_period: u64,
    pub max_per_use: u64,
    pub enforce_exact_quantity: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub struct UsageState {
    pub remaining_in_period: u64,
    pub last_reset: i64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SpendingLimitV2 {
    pub mint: Address,
    pub time_constraints: TimeConstraints,
    pub quantity_constraints: QuantityConstraints,
    pub usage: UsageState,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SpendingLimitPolicy {
    pub source_account_index: u8,
    pub destinations: Vec<Address>,
    pub spending_limit: SpendingLimitV2,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SpendingLimitPolicyCreationPayload {
    pub mint: Address,
    pub source_account_index: u8,
    pub time_constraints: TimeConstraints,
    pub quantity_constraints: QuantityConstraints,
    pub usage_state: Option<UsageState>,
    pub destinations: Vec<Address>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SpendingLimitPayload {
    pub amount: u64,
    pub destination: Address,
    pub decimals: u8,
}

// ---------------------------------------------------------------------------
// Settings-change policy
// ---------------------------------------------------------------------------

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SettingsChangePolicy {
    pub actions: Vec<AllowedSettingsChange>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum AllowedSettingsChange {
    AddSigner {
        new_signer: Option<Address>,
        new_signer_permissions: Option<Permissions>,
    },
    RemoveSigner {
        old_signer: Option<Address>,
    },
    ChangeThreshold,
    ChangeTimeLock {
        new_time_lock: Option<u32>,
    },
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SettingsChangePolicyCreationPayload {
    pub actions: Vec<AllowedSettingsChange>,
}

/// Subset of [`super::snapshots::SettingsActionFull`] permitted under
/// `SettingsChange` policies.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum LimitedSettingsAction {
    AddSigner { new_signer: SmartAccountSigner },
    RemoveSigner { old_signer: Address },
    ChangeThreshold { new_threshold: u16 },
    SetTimeLock { new_time_lock: u32 },
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SettingsChangePayload {
    pub action_index: Vec<u8>,
    pub actions: Vec<LimitedSettingsAction>,
}

// ---------------------------------------------------------------------------
// Program-interaction policy
// ---------------------------------------------------------------------------

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct ProgramInteractionPolicy {
    pub account_index: u8,
    pub instructions_constraints: Vec<InstructionConstraint>,
    pub pre_hook: Option<Hook>,
    pub post_hook: Option<Hook>,
    pub spending_limits: Vec<SpendingLimitV2>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct InstructionConstraint {
    pub program_id: Address,
    pub account_constraints: Vec<AccountConstraint>,
    pub data_constraints: Vec<DataConstraint>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct Hook {
    pub num_extra_accounts: u8,
    pub account_constraints: Vec<AccountConstraint>,
    pub instruction_data: Vec<u8>,
    pub program_id: Address,
    pub pass_inner_instructions: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum DataOperator {
    Equals,
    NotEquals,
    GreaterThan,
    GreaterThanOrEqualTo,
    LessThan,
    LessThanOrEqualTo,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum DataValue {
    U8(u8),
    U16Le(u16),
    U32Le(u32),
    U64Le(u64),
    U128Le(u128),
    U8Slice(Vec<u8>),
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct DataConstraint {
    pub data_offset: u64,
    pub data_value: DataValue,
    pub operator: DataOperator,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum AccountConstraintType {
    Address(Vec<Address>),
    AccountData(Vec<DataConstraint>),
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct AccountConstraint {
    pub account_index: u8,
    pub account_constraint: AccountConstraintType,
    pub owner: Option<Address>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct LimitedTimeConstraints {
    pub start: i64,
    pub expiration: Option<i64>,
    pub period: PeriodV2,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct LimitedQuantityConstraints {
    pub max_per_period: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct LimitedSpendingLimit {
    pub mint: Address,
    pub time_constraints: LimitedTimeConstraints,
    pub quantity_constraints: LimitedQuantityConstraints,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct ProgramInteractionPolicyCreationPayload {
    pub account_index: u8,
    pub instructions_constraints: Vec<InstructionConstraint>,
    pub pre_hook: Option<Hook>,
    pub post_hook: Option<Hook>,
    pub spending_limits: Vec<LimitedSpendingLimit>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct ProgramInteractionPayload {
    pub instruction_constraint_indices: Option<Vec<u8>>,
    pub transaction_payload: ProgramInteractionTransactionPayload,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum ProgramInteractionTransactionPayload {
    AsyncTransaction(crate::helpers::events::snapshots::AsyncTransactionPayload),
    SyncTransaction(SyncTransactionPayloadDetails),
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SyncTransactionPayloadDetails {
    pub account_index: u8,
    pub instructions: Vec<u8>,
}
