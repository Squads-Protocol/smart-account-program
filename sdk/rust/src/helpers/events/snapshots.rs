//! Account-state snapshot mirror types used inside event payloads.
//!
//! The program's `#[account]` structs (Settings, Transaction, Proposal,
//! SpendingLimit, SettingsTransaction) are borsh-encoded **without** the
//! 8-byte anchor discriminator when they appear inside event payloads (raw
//! `try_to_vec` on the struct, not via anchor's `AccountSerialize`).
//!
//! Each codama-emitted account struct has a leading `discriminator: [u8; 8]`
//! field, so we can't reuse them directly for event payloads — these
//! snapshots are discriminator-less mirrors of the same fields.
//!
//! For policy-related types and the `Payload` enum, re-exports from
//! `crate::generated::types` are used directly (those types don't have the
//! discriminator-field issue because they're not accounts).

use borsh::{BorshDeserialize, BorshSerialize};
use solana_address::Address;

use crate::generated::types::{
    Period, ProposalStatus, SettingsAction, SmartAccountSigner, SmartAccountTransactionMessage,
};

// Re-export the codama-emitted Payload tree — these match the program's
// runtime emission exactly (no discriminator difference) so no snapshot
// mirror is needed.
pub use crate::generated::types::{
    Payload, PolicyActionPayloadDetails, TransactionPayload, TransactionPayloadDetails,
};

/// Discriminates Settings vs Policy consensus accounts. Not emitted by codama
/// (no IDL surface — only used inside the program's `ConsensusAccount`
/// interface wrapper), so defined locally.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsensusAccountType {
    Settings,
    Policy,
}

/// Mirror of [`crate::generated::accounts::Settings`] without the
/// `discriminator` field. Use this in event payloads.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SettingsSnapshot {
    pub seed: u128,
    pub settings_authority: Address,
    pub threshold: u16,
    pub time_lock: u32,
    pub transaction_index: u64,
    pub stale_transaction_index: u64,
    pub archival_authority: Option<Address>,
    pub archivable_after: u64,
    pub bump: u8,
    pub signers: Vec<SmartAccountSigner>,
    pub account_utilization: u8,
    pub policy_seed: Option<u64>,
    pub reserved2: u8,
}

/// Mirror of [`crate::generated::accounts::Proposal`] without the
/// `discriminator` field.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct ProposalSnapshot {
    pub settings: Address,
    pub transaction_index: u64,
    pub rent_collector: Address,
    pub status: ProposalStatus,
    pub bump: u8,
    pub approved: Vec<Address>,
    pub rejected: Vec<Address>,
    pub cancelled: Vec<Address>,
}

/// Mirror of [`crate::generated::accounts::Transaction`] without the
/// `discriminator` field. Wraps the codama-emitted `Payload` enum directly.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct TransactionSnapshot {
    pub consensus_account: Address,
    pub creator: Address,
    pub rent_collector: Address,
    pub index: u64,
    pub payload: Payload,
}

/// Mirror of [`crate::generated::accounts::SpendingLimit`] without the
/// `discriminator` field.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SpendingLimitSnapshot {
    pub settings: Address,
    pub seed: Address,
    pub account_index: u8,
    pub mint: Address,
    pub amount: u64,
    pub period: Period,
    pub remaining_amount: u64,
    pub last_reset: i64,
    pub bump: u8,
    pub signers: Vec<Address>,
    pub destinations: Vec<Address>,
    pub expiration: i64,
}

/// Mirror of [`crate::generated::accounts::SettingsTransaction`] without the
/// `discriminator` field. Uses the codama-emitted `SettingsAction` enum which
/// now includes all 10 variants (Policy* included).
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SettingsTransactionSnapshot {
    pub settings: Address,
    pub creator: Address,
    pub rent_collector: Address,
    pub index: u64,
    pub bump: u8,
    pub actions: Vec<SettingsAction>,
}

#[allow(dead_code)]
fn _unused() {
    // Keep imports live across feature flag combinations.
    let _: Option<SmartAccountTransactionMessage> = None;
}
