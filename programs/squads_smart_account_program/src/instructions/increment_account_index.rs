use anchor_lang::prelude::*;

use crate::{
    errors::SmartAccountError,
    events::{IncrementAccountIndexEvent, LogAuthorityInfo, SmartAccountEvent},
    interface::consensus_trait::Consensus,
    program::SquadsSmartAccountProgram,
    state::{
        get_settings_signer_seeds, ClientDataJsonReconstructionParams, Permission, Settings, FREE_ACCOUNT_MAX_INDEX,
        SEED_PREFIX, SEED_SETTINGS,
    },
    utils::{create_increment_account_index_message, verify_v2_context},
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct IncrementAccountIndexV2Args {
    pub signer_key: Pubkey,
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
}

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
}

impl IncrementAccountIndex<'_> {
    fn validate(&self) -> Result<()> {
        let settings = &self.settings;
        let signer_key = self.signer.key();

        Self::validate_signer(settings, signer_key)
    }

    fn validate_signer(settings: &Settings, signer_key: Pubkey) -> Result<()> {
        let signer = settings
            .signers
            .find(&signer_key)
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

impl IncrementAccountIndexV2<'_> {
    fn validate(&self, args: &IncrementAccountIndexV2Args) -> Result<()> {
        IncrementAccountIndex::validate_signer(&self.settings, args.signer_key)
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn increment_account_index_v2(
        ctx: Context<Self>,
        args: IncrementAccountIndexV2Args,
    ) -> Result<()> {
        let expected_message =
            create_increment_account_index_message(&ctx.accounts.settings.key(), args.signer_key);

        verify_v2_context(
            &mut ctx.accounts.settings,
            args.signer_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        let settings = &mut ctx.accounts.settings;
        settings.account_utilization = settings.account_utilization.checked_add(1).unwrap();
        Ok(())
    }
}
