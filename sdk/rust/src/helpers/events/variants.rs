//! The 13 event variant structs that get wrapped by [`SmartAccountEvent`].
//!
//! Field shapes match the program's `events/account_events.rs` exactly so
//! borsh decoding round-trips on the bytes emitted by the program's
//! `LogEvent` self-CPI.

use borsh::{BorshDeserialize, BorshSerialize};
use solana_address::Address;

use crate::generated::types::SmartAccountCompiledInstruction;
use crate::helpers::events::policy::{LimitedSettingsAction, Policy, PolicyPayload};
use crate::helpers::events::snapshots::{
    ConsensusAccountType, ProposalSnapshot, SettingsActionFull, SettingsSnapshot,
    SettingsTransactionSnapshot, SpendingLimitSnapshot, TransactionSnapshot,
};

// 0
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct CreateSmartAccountEvent {
    pub new_settings_pubkey: Address,
    pub new_settings_content: SettingsSnapshot,
}

// 1
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SynchronousTransactionEvent {
    pub settings_pubkey: Address,
    pub account_index: u8,
    pub signers: Vec<Address>,
    pub instructions: Vec<SmartAccountCompiledInstruction>,
    pub instruction_accounts: Vec<Address>,
}

// 2
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SynchronousSettingsTransactionEvent {
    pub settings_pubkey: Address,
    pub signers: Vec<Address>,
    pub settings: SettingsSnapshot,
    pub changes: Vec<SettingsActionFull>,
}

// 3
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct AddSpendingLimitEvent {
    pub settings_pubkey: Address,
    pub spending_limit_pubkey: Address,
    pub spending_limit: SpendingLimitSnapshot,
}

// 4
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct RemoveSpendingLimitEvent {
    pub settings_pubkey: Address,
    pub spending_limit_pubkey: Address,
}

// 5
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct UseSpendingLimitEvent {
    pub settings_pubkey: Address,
    pub spending_limit_pubkey: Address,
    pub smart_account: Address,
    pub smart_account_token_account: Address,
    pub destination: Address,
    pub destination_token_account: Address,
    pub signer: Address,
    pub mint: Address,
    pub mint_decimals: u8,
    pub amount: u64,
    pub spending_limit: SpendingLimitSnapshot,
}

// 6
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct AuthoritySettingsEvent {
    pub settings: SettingsSnapshot,
    pub settings_pubkey: Address,
    pub authority: Address,
    pub change: SettingsActionFull,
}

// 7
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct AuthorityChangeEvent {
    pub settings: SettingsSnapshot,
    pub settings_pubkey: Address,
    pub authority: Address,
    pub new_authority: Option<Address>,
}

// 8
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct TransactionEvent {
    pub consensus_account: Address,
    pub consensus_account_type: ConsensusAccountType,
    pub event_type: TransactionEventType,
    pub transaction_pubkey: Address,
    pub transaction_index: u64,
    pub signer: Option<Address>,
    pub memo: Option<String>,
    pub transaction_content: Option<TransactionContent>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum TransactionContent {
    Transaction(TransactionSnapshot),
    SettingsTransaction {
        settings: SettingsSnapshot,
        transaction: SettingsTransactionSnapshot,
        changes: Vec<SettingsActionFull>,
    },
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum TransactionEventType {
    Create,
    Execute,
    Close,
}

// 9
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct ProposalEvent {
    pub consensus_account: Address,
    pub consensus_account_type: ConsensusAccountType,
    pub event_type: ProposalEventType,
    pub proposal_pubkey: Address,
    pub transaction_index: u64,
    pub signer: Option<Address>,
    pub memo: Option<String>,
    pub proposal: Option<ProposalSnapshot>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum ProposalEventType {
    Create,
    Approve,
    Reject,
    Cancel,
    Execute,
    Close,
}

// 10
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SynchronousTransactionEventV2 {
    pub consensus_account: Address,
    pub consensus_account_type: ConsensusAccountType,
    pub signers: Vec<Address>,
    pub payload: SynchronousTransactionEventPayload,
    pub instruction_accounts: Vec<Address>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum SynchronousTransactionEventPayload {
    TransactionPayload {
        account_index: u8,
        instructions: Vec<SmartAccountCompiledInstruction>,
    },
    PolicyPayload {
        policy_payload: PolicyPayload,
    },
}

// 11
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SettingsChangePolicyEvent {
    pub settings_pubkey: Address,
    pub settings: SettingsSnapshot,
    pub changes: Vec<LimitedSettingsAction>,
}

// 12
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct PolicyEvent {
    pub event_type: PolicyEventType,
    pub settings_pubkey: Address,
    pub policy_pubkey: Address,
    pub policy: Option<Policy>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum PolicyEventType {
    Create,
    Update,
    UpdateDuringExecution,
    Remove,
}
