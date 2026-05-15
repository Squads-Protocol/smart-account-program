//! Policy types still needed in helpers after the IDL refresh.
//!
//! The anchor 0.30+ IDL emits most of the policy data tree (PolicyPayload,
//! PolicyCreationPayload, the 4 implementations' payload structs, etc.). What
//! anchor's IDL builder does NOT see are types only used inside the on-chain
//! `Policy` account's storage — `Policy` itself isn't surfaced as a top-level
//! account in the IDL because the program only ever loads it through the
//! `InterfaceAccount<'info, ConsensusAccount>` interface wrapper.
//!
//! So this module hosts the program-side Policy storage types (Policy struct,
//! PolicyState enum, PolicyExpiration enum, and the 4 *Policy state structs
//! they contain). Everything else is re-exported from `crate::generated::types`.

use borsh::{BorshDeserialize, BorshSerialize};
use solana_address::Address;

use crate::generated::types::{
    AllowedSettingsChange, LimitedTimeConstraints, PeriodV2, QuantityConstraints,
    SmartAccountSigner, TimeConstraints, UsageState,
};

pub use crate::generated::types::{
    AccountConstraint, AccountConstraintType, DataConstraint, DataOperator, DataValue, Hook,
    InstructionConstraint, InternalFundTransferPayload, InternalFundTransferPolicyCreationPayload,
    LimitedQuantityConstraints, LimitedSettingsAction, LimitedSpendingLimit, PolicyCreationPayload,
    PolicyExpirationArgs, PolicyPayload, ProgramInteractionPayload,
    ProgramInteractionPolicyCreationPayload, ProgramInteractionTransactionPayload,
    SettingsChangePayload, SettingsChangePolicyCreationPayload, SpendingLimitPayload,
    SpendingLimitPolicyCreationPayload, SyncTransactionPayloadDetails,
};

// ---------------------------------------------------------------------------
// Storage-only types (not in IDL because Policy account isn't surfaced)
// ---------------------------------------------------------------------------

/// Mirror of the program's `state/policies/policy_core/policy.rs::Policy`
/// (no discriminator field — the on-chain account stores anchor's 8-byte
/// discriminator separately but the in-memory struct doesn't).
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

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum PolicyState {
    InternalFundTransfer(InternalFundTransferPolicy),
    SpendingLimit(SpendingLimitPolicy),
    SettingsChange(SettingsChangePolicy),
    ProgramInteraction(ProgramInteractionPolicy),
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum PolicyExpiration {
    Timestamp(i64),
    SettingsState([u8; 32]),
}

// The four *Policy storage states embedded inside PolicyState. Codama emits
// the *PolicyCreationPayload (input) and *Payload (execution) variants of
// each but not the *Policy (storage) shapes.

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct InternalFundTransferPolicy {
    pub source_account_mask: [u8; 32],
    pub destination_account_mask: [u8; 32],
    pub allowed_mints: Vec<Address>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SpendingLimitPolicy {
    pub source_account_index: u8,
    pub destinations: Vec<Address>,
    pub spending_limit: SpendingLimitV2,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SettingsChangePolicy {
    pub actions: Vec<AllowedSettingsChange>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct ProgramInteractionPolicy {
    pub account_index: u8,
    pub instructions_constraints: Vec<InstructionConstraint>,
    pub pre_hook: Option<Hook>,
    pub post_hook: Option<Hook>,
    pub spending_limits: Vec<SpendingLimitV2>,
}

/// Stored spending limit (with usage state) — sister of the standalone
/// `SpendingLimit` account. Not in IDL because it's only used as a nested
/// type inside Policy's storage.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SpendingLimitV2 {
    pub mint: Address,
    pub time_constraints: TimeConstraints,
    pub quantity_constraints: QuantityConstraints,
    pub usage: UsageState,
}

// Suppress unused-import warnings on the codama re-exports above — they're
// brought into scope so callers can `use helpers::events::policy::*;` and
// get the full type catalogue.
#[allow(dead_code)]
fn _unused() {
    let _: Option<LimitedTimeConstraints> = None;
    let _: Option<PeriodV2> = None;
}
