//! Account-state snapshot mirror types used inside event payloads.
//!
//! The program's `#[account]` structs (Settings, Transaction, Proposal,
//! SpendingLimit, SettingsTransaction) are borsh-encoded **without** the
//! 8-byte anchor discriminator when they appear inside event payloads (raw
//! `try_to_vec` on the struct, not via anchor's `AccountSerialize`). These
//! mirror types capture that exact on-wire shape.
//!
//! These are NOT the same as `crate::generated::accounts::*`:
//! - they drop the leading `discriminator: [u8; 8]` field
//! - they reflect the *current* program source (which can drift from the IDL
//!   if it hasn't been regenerated)

use borsh::{BorshDeserialize, BorshSerialize};
use solana_address::Address;

use crate::generated::types::{Period, ProposalStatus, SmartAccountSigner, SmartAccountTransactionMessage};
use crate::helpers::events::policy::{PolicyCreationPayload, PolicyExpirationArgs, PolicyPayload};

/// Discriminates Settings vs Policy consensus accounts.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsensusAccountType {
    Settings,
    Policy,
}

/// Mirror of the program's `state/settings.rs::Settings` (no discriminator).
///
/// **Field layout differs from `crate::generated::accounts::Settings`** —
/// codama emits `reserved1: u8, reserved2: u8` but the current program has
/// `policy_seed: Option<u64>, _reserved2: u8`. This snapshot uses the program
/// shape since that's what gets emitted in events.
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
    pub _reserved2: u8,
}

/// Mirror of the program's `state/proposal.rs::Proposal` (no discriminator).
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

/// Mirror of the program's `state/transaction.rs::Transaction` (no
/// discriminator). The program uses a `payload: Payload` enum that covers
/// both transaction execution and policy execution; codama's emitted
/// `Transaction` flattens this to `{settings, message}`, which is wrong.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct TransactionSnapshot {
    pub consensus_account: Address,
    pub creator: Address,
    pub rent_collector: Address,
    pub index: u64,
    pub payload: Payload,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum Payload {
    TransactionPayload(TransactionPayloadDetails),
    PolicyPayload(PolicyActionPayloadDetails),
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct TransactionPayloadDetails {
    pub account_index: u8,
    pub ephemeral_signer_bumps: Vec<u8>,
    pub message: SmartAccountTransactionMessage,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct PolicyActionPayloadDetails {
    pub payload: PolicyPayload,
}

/// Mirror of the program's `state/spending_limit.rs::SpendingLimit`.
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

/// Mirror of the program's `state/settings_transaction.rs::SettingsTransaction`.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SettingsTransactionSnapshot {
    pub settings: Address,
    pub creator: Address,
    pub rent_collector: Address,
    pub index: u64,
    pub bump: u8,
    pub actions: Vec<SettingsActionFull>,
}

/// Program-side `SettingsAction` enum (10 variants).
///
/// **Differs from `crate::generated::types::SettingsAction`** — codama emits
/// 7 variants (missing PolicyCreate, PolicyUpdate, PolicyRemove). The full
/// version is needed for event decoding because the program emits all 10.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum SettingsActionFull {
    AddSigner {
        new_signer: SmartAccountSigner,
    },
    RemoveSigner {
        old_signer: Address,
    },
    ChangeThreshold {
        new_threshold: u16,
    },
    SetTimeLock {
        new_time_lock: u32,
    },
    AddSpendingLimit {
        seed: Address,
        account_index: u8,
        mint: Address,
        amount: u64,
        period: Period,
        signers: Vec<Address>,
        destinations: Vec<Address>,
        expiration: i64,
    },
    RemoveSpendingLimit {
        spending_limit: Address,
    },
    SetArchivalAuthority {
        new_archival_authority: Option<Address>,
    },
    PolicyCreate {
        seed: u64,
        policy_creation_payload: PolicyCreationPayload,
        signers: Vec<SmartAccountSigner>,
        threshold: u16,
        time_lock: u32,
        start_timestamp: Option<i64>,
        expiration_args: Option<PolicyExpirationArgs>,
    },
    PolicyUpdate {
        policy: Address,
        signers: Vec<SmartAccountSigner>,
        threshold: u16,
        time_lock: u32,
        policy_update_payload: PolicyCreationPayload,
        expiration_args: Option<PolicyExpirationArgs>,
    },
    PolicyRemove {
        policy: Address,
    },
}

/// `TransactionPayload` from the program's `transaction_create` arg —
/// used inside `ProgramInteractionTransactionPayload::AsyncTransaction`.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct AsyncTransactionPayload {
    pub account_index: u8,
    pub ephemeral_signers: u8,
    pub transaction_message: Vec<u8>,
    pub memo: Option<String>,
}

