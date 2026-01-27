use anchor_lang::prelude::*;

use crate::{
    errors::SmartAccountError,
    events::{IncrementAccountIndexEvent, LogAuthorityInfo, SmartAccountEvent},
    interface::consensus_trait::Consensus,
    program::SquadsSmartAccountProgram,
    state::{
        get_settings_signer_seeds, Permission, Settings, FREE_ACCOUNT_MAX_INDEX, SEED_PREFIX,
        SEED_SETTINGS,
    },
};

#[derive(Accounts)]
pub struct IncrementAccountIndex<'info> {
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            SEED_SETTINGS,
            &settings.seed.to_le_bytes(),
        ],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    pub signer: Signer<'info>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl IncrementAccountIndex<'_> {
    fn validate(&self) -> Result<()> {
        let settings = &self.settings;
        let signer_key = self.signer.key();

        // Signer must be a member of the smart account
        let signer_index = settings
            .is_signer(signer_key)
            .ok_or(SmartAccountError::NotASigner)?;

        // Permission: Initiate OR Vote OR Execute (mask & 7 != 0)
        let permissions = settings.signers[signer_index].permissions;
        require!(
            permissions.has(Permission::Initiate)
                || permissions.has(Permission::Vote)
                || permissions.has(Permission::Execute),
            SmartAccountError::Unauthorized
        );

        // Cannot exceed free account range
        require!(
            settings.account_utilization < FREE_ACCOUNT_MAX_INDEX,
            SmartAccountError::MaxAccountIndexReached
        );

        Ok(())
    }

    #[access_control(ctx.accounts.validate())]
    pub fn increment_account_index(ctx: Context<Self>) -> Result<()> {
        let settings = &mut ctx.accounts.settings;
        settings.account_utilization = settings.account_utilization.checked_add(1).unwrap();

        let event = IncrementAccountIndexEvent {
            settings_pubkey: settings.key(),
            settings_state: settings.clone().into_inner(),
        };
        let log_authority_info = LogAuthorityInfo {
            authority: settings.to_account_info(),
            authority_seeds: get_settings_signer_seeds(settings.seed),
            bump: settings.bump,
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::IncrementAccountIndexEvent(event).log(&log_authority_info)?;
        Ok(())
    }
}
