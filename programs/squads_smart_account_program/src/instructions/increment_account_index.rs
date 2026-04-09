use anchor_lang::prelude::*;
use crate::instructions::*;

use crate::{
    errors::SmartAccountError,
    events::{IncrementAccountIndexEvent, LogAuthorityInfo, SmartAccountEvent},
    interface::consensus_trait::Consensus,
    program::SquadsSmartAccountProgram,
    state::{
        get_settings_signer_seeds, Permission, ResolvedSigner, ResolvedSignerBumps, Settings, FREE_ACCOUNT_MAX_INDEX, SEED_PREFIX,
        SEED_SETTINGS,
    },
    state::signer_v2::ExtraVerificationData,
    state::signer_v2::precompile::create_increment_account_index_message,
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
        let signer = settings
            .is_signer_v2(signer_key)
            .ok_or(SmartAccountError::NotASigner)?;

        // Permission: Initiate OR Vote OR Execute (mask & 7 != 0)
        let permissions = signer.permissions();
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
        settings.increment_account_utilization_index()?;

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

// =========================================================================
// V2: External signer support
// =========================================================================

#[derive(Accounts)]
pub struct IncrementAccountIndexV2<'info> {
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

    pub signer: ResolvedSigner<'info>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl IncrementAccountIndexV2<'_> {
    fn validate(
        &mut self,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: &Option<ExtraVerificationData>,
    ) -> Result<()> {
        let message = create_increment_account_index_message(
            &self.settings.key(),
            self.signer.key(),
        );

        // Resolve and verify signer (native, session key, or external)
        self.signer.verify(
            &mut *self.settings,
            remaining_accounts,
            message,
            extra_verification_data.as_ref(),
            None, // Check permission manually (OR logic)
        )?;

        // Permission: Initiate OR Vote OR Execute
        let resolved_key = self.signer.resolved_key()?;
        let signer = self.settings
            .is_signer_v2(resolved_key)
            .ok_or(SmartAccountError::NotASigner)?;
        let permissions = signer.permissions();
        require!(
            permissions.has(Permission::Initiate)
                || permissions.has(Permission::Vote)
                || permissions.has(Permission::Execute),
            SmartAccountError::Unauthorized
        );

        // Cannot exceed free account range
        require!(
            self.settings.account_utilization < FREE_ACCOUNT_MAX_INDEX,
            SmartAccountError::MaxAccountIndexReached
        );

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&ctx.remaining_accounts, &extra_verification_data))]
    pub fn increment_account_index_v2(
        ctx: Context<Self>,
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        let settings = &mut ctx.accounts.settings;
        settings.increment_account_utilization_index()?;

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
