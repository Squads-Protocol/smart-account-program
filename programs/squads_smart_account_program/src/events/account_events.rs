use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};
use crate::{state::SettingsAction, Proposal, Settings, SmartAccountCompiledInstruction, SpendingLimit, Transaction};


#[derive(BorshSerialize, BorshDeserialize)]
pub struct CreateSmartAccountEvent {
    pub new_settings_pubkey: Pubkey,
    pub new_settings_content: Settings,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct SynchronousTransactionEvent {
    pub settings_pubkey: Pubkey,
    pub account_index: u8,
    pub signers: Vec<Pubkey>,
    pub instructions: Vec<SmartAccountCompiledInstruction>,
    pub instruction_accounts: Vec<Pubkey>,
}


#[derive(BorshSerialize, BorshDeserialize)]
pub struct SynchronousSettingsTransactionEvent {
    pub settings_pubkey: Pubkey,
    pub signers: Vec<Pubkey>,
    pub settings: Settings,
    pub changes: Vec<SettingsAction>
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AddSpendingLimitEvent {
    pub settings_pubkey: Pubkey,
    pub spending_limit_pubkey: Pubkey,
    pub spending_limit: SpendingLimit,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct RemoveSpendingLimitEvent {
    pub settings_pubkey: Pubkey,
    pub spending_limit_pubkey: Pubkey,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct UseSpendingLimitEvent {
    pub settings_pubkey: Pubkey,
    pub spending_limit_pubkey: Pubkey,
    pub smart_account: Pubkey,
    pub smart_account_token_account: Pubkey,
    pub destination: Pubkey,
    pub destination_token_account: Pubkey,
    pub signer: Pubkey,
    pub mint: Pubkey,
    pub mint_decimals: u8,
    pub amount: u64,
    pub spending_limit: SpendingLimit,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AuthoritySettingsEvent {
    pub settings: Settings,
    pub settings_pubkey: Pubkey,
    pub authority: Pubkey,
    pub change: SettingsAction
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct AuthorityChangeEvent {
    pub settings: Settings,
    pub settings_pubkey: Pubkey,
    pub authority: Pubkey,
    pub new_authority: Option<Pubkey>
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct TransactionEvent {
    pub settings_pubkey: Pubkey,
    pub event_type: TransactionEventType,
    pub transaction_pubkey: Pubkey,
    pub transaction_index: u64,
    pub signer: Option<Pubkey>,
    pub memo: Option<String>,
    pub transaction: Option<Transaction>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum TransactionEventType {
    Create,
    Execute,
    Close
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct ProposalEvent {
    pub settings_pubkey: Pubkey,
    pub event_type: ProposalEventType,
    pub proposal_pubkey: Pubkey,
    pub transaction_index: u64,
    pub signer: Option<Pubkey>,
    pub memo: Option<String>,
    pub proposal: Option<Proposal>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum ProposalEventType {
    Create,
    Approve,
    Reject,
    Cancel,
    Execute,
    Close
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct TransactionExecuteEvent {
    pub settings_pubkey: Pubkey,
    pub proposal_pubkey: Pubkey,
    pub transaction_pubkey: Pubkey,
    pub transaction_index: u64,
    pub executor: Pubkey,
    pub account_index: u8,
}