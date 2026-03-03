use anchor_lang::prelude::*;
use anchor_lang::solana_program::pubkey;

use crate::{
    errors::SmartAccountError,
    events::*,
    program::SquadsSmartAccountProgram,
    state::{Settings, SEED_PREFIX, SEED_SETTINGS, get_settings_signer_seeds},
};

pub const FREE_ACCOUNT_MAX_INDEX: u8 = 249;

#[cfg(not(feature = "testing"))]
const PAYMASTER: Pubkey = pubkey!("7kEydiJ9en86ESNpwpZ45khxX8usgjUmWxbSTzkSbXkR");

#[cfg(feature = "testing")]
const PAYMASTER: Pubkey = pubkey!("BHpoHAaFDFPwDP47mcpy2R4fXX66NsUe8y8RFkfrEMNG");

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SetAccountIndexArgs {
    pub new_index: u8,
}

#[derive(Accounts)]
pub struct SetAccountIndex<'info> {
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

    #[account(
        address = PAYMASTER @ SmartAccountError::Unauthorized
    )]
    pub paymaster: Signer<'info>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl SetAccountIndex<'_> {
    fn validate(&self, args: &SetAccountIndexArgs) -> Result<()> {
        require!(
            args.new_index <= FREE_ACCOUNT_MAX_INDEX,
            SmartAccountError::InvalidInstructionArgs
        );
        Ok(())
    }

    #[access_control(ctx.accounts.validate(&args))]
    pub fn set_account_index(ctx: Context<Self>, args: SetAccountIndexArgs) -> Result<()> {
        let settings = &mut ctx.accounts.settings;
        settings.account_utilization = args.new_index;

        let event = SetAccountIndexEvent {
            settings: Settings::try_from_slice(&settings.try_to_vec()?)?,
            settings_pubkey: settings.key(),
        };
        let log_authority_info = LogAuthorityInfo {
            authority: settings.to_account_info(),
            authority_seeds: get_settings_signer_seeds(settings.seed),
            bump: settings.bump,
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::SetAccountIndexEvent(event).log(&log_authority_info)?;

        Ok(())
    }
}
