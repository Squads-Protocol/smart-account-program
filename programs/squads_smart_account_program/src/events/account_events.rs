use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{state::SettingsAction, Settings, SmartAccountCompiledInstruction, SpendingLimit};


#[derive(BorshSerialize, BorshDeserialize)]
pub struct CreateSmartAccountEvent {
    pub new_settings_pubkey: Pubkey,
    pub new_settings_content: Settings,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct SynchronousTransactionEvent {
    pub settings_pubkey: Pubkey,
    pub account_index: u8,
    pub instructions: Vec<SmartAccountCompiledInstruction>,
    pub instruction_accounts: Vec<Pubkey>,
}


#[derive(BorshSerialize, BorshDeserialize)]
pub struct SynchronousSettingsTransactionEvent {
    pub settings_pubkey: Pubkey,
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
    pub spending_limit: SpendingLimit,
}