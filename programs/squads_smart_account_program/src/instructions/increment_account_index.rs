use anchor_lang::prelude::*;
use anchor_lang::solana_program::pubkey;

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

// TODO: update before mainnet
#[cfg(not(feature = "testing"))]
const INCREMENT_AUTHORITY: Pubkey = pubkey!("11111111111111111111111111111111");

#[cfg(feature = "testing")]
const INCREMENT_AUTHORITY: Pubkey = pubkey!("DuFcCkArwSCTZ516uFbUcBYUM9TQdAo4xNDLWrzUnDGW");

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

        // Cannot exceed free account range
        require!(
            settings.account_utilization < FREE_ACCOUNT_MAX_INDEX,
            SmartAccountError::MaxAccountIndexReached
        );

        // Increment authority can always increment
        if signer_key == INCREMENT_AUTHORITY {
            return Ok(());
        }

        // Member with any permission
        let signer_index = settings
            .is_signer(signer_key)
            .ok_or(SmartAccountError::NotASigner)?;

        let permissions = settings.signers[signer_index].permissions;
        require!(
            permissions.has(Permission::Initiate)
                || permissions.has(Permission::Vote)
                || permissions.has(Permission::Execute),
            SmartAccountError::Unauthorized
        );

        Ok(())
    }

    #[access_control(ctx.accounts.validate())]
    pub fn increment_account_index(ctx: Context<Self>) -> Result<()> {
        let settings = &mut ctx.accounts.settings;
        settings.increment_account_utilization_index();

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
