use anchor_lang::prelude::*;

use crate::{
    errors::*,
    program::SquadsSmartAccountProgram,
    state::*,
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RemoveExternalSignerArgs {
    /// The key_id of the signer to remove
    pub key_id: Pubkey,
    /// Optional memo for indexing
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct SettingsRemoveExternalSigner<'info> {
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    /// Payer for rent return
    #[account(mut)]
    pub payer: Signer<'info>,

    /// System program
    pub system_program: Program<'info, System>,

    #[allow(unused)]
    pub program: Program<'info, SquadsSmartAccountProgram>,

    // remaining_accounts:
    // - FIRST: instructions sysvar (sysvar::instructions::ID) - if external sigs used
    // - Rest: native signer accounts for consensus
}

impl<'info> SettingsRemoveExternalSigner<'info> {
    /// Remove an external signer from the smart account
    /// 
    /// NOTE: This instruction is currently not implemented as it requires migrating
    /// the Settings account to use SmartAccountSignerWrapper instead of Vec<SmartAccountSigner>.
    pub fn settings_remove_external_signer(
        _ctx: Context<SettingsRemoveExternalSigner<'info>>,
        _args: RemoveExternalSignerArgs,
    ) -> Result<()> {
        // Not yet implemented - requires Settings migration to SmartAccountSignerWrapper
        Err(SmartAccountError::NotImplemented.into())
    }
}

#[event]
pub struct ExternalSignerRemovedEvent {
    pub settings: Pubkey,
    pub key_id: Pubkey,
    pub signer_type: u8,
}
