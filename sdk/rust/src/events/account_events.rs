use solana_address::Address;

use crate::interface::ConsensusAccountType;
use crate::state::{
    LimitedSettingsAction, Policy, PolicyPayload, Proposal, Settings, SettingsAction,
    SettingsTransaction, SmartAccountCompiledInstruction, SpendingLimit, Transaction,
};

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct CreateSmartAccountEvent {
    pub new_settings_pubkey: Address,
    pub new_settings_content: Settings,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct SynchronousTransactionEventV2 {
    pub consensus_account: Address,
    pub consensus_account_type: ConsensusAccountType,
    pub signers: Vec<Address>,
    pub payload: SynchronousTransactionEventPayload,
    pub instruction_accounts: Vec<Address>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub enum SynchronousTransactionEventPayload {
    TransactionPayload {
        account_index: u8,
        instructions: Vec<SmartAccountCompiledInstruction>,
    },
    PolicyPayload {
        policy_payload: PolicyPayload,
    },
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct SynchronousTransactionEvent {
    pub settings_pubkey: Address,
    pub account_index: u8,
    pub signers: Vec<Address>,
    pub instructions: Vec<SmartAccountCompiledInstruction>,
    pub instruction_accounts: Vec<Address>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct SynchronousSettingsTransactionEvent {
    pub settings_pubkey: Address,
    pub signers: Vec<Address>,
    pub settings: Settings,
    pub changes: Vec<SettingsAction>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct AddSpendingLimitEvent {
    pub settings_pubkey: Address,
    pub spending_limit_pubkey: Address,
    pub spending_limit: SpendingLimit,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct RemoveSpendingLimitEvent {
    pub settings_pubkey: Address,
    pub spending_limit_pubkey: Address,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct PolicyEvent {
    pub event_type: PolicyEventType,
    pub settings_pubkey: Address,
    pub policy_pubkey: Address,
    pub policy: Option<Policy>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub enum PolicyEventType {
    Create,
    Update,
    UpdateDuringExecution,
    Remove,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
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
    pub spending_limit: SpendingLimit,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct AuthoritySettingsEvent {
    pub settings: Settings,
    pub settings_pubkey: Address,
    pub authority: Address,
    pub change: SettingsAction,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct AuthorityChangeEvent {
    pub settings: Settings,
    pub settings_pubkey: Address,
    pub authority: Address,
    pub new_authority: Option<Address>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
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

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub enum TransactionContent {
    Transaction(Transaction),
    SettingsTransaction {
        settings: Settings,
        transaction: SettingsTransaction,
        changes: Vec<SettingsAction>,
    },
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub enum TransactionEventType {
    Create,
    Execute,
    Close,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct ProposalEvent {
    pub consensus_account: Address,
    pub consensus_account_type: ConsensusAccountType,
    pub event_type: ProposalEventType,
    pub proposal_pubkey: Address,
    pub transaction_index: u64,
    pub signer: Option<Address>,
    pub memo: Option<String>,
    pub proposal: Option<Proposal>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub enum ProposalEventType {
    Create,
    Approve,
    Reject,
    Cancel,
    Execute,
    Close,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct SettingsChangePolicyEvent {
    pub settings_pubkey: Address,
    pub settings: Settings,
    pub changes: Vec<LimitedSettingsAction>,
}
