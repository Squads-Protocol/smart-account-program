use anchor_lang::prelude::*;

use crate::errors::*;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;
use crate::AuthoritySettingsEvent;
use crate::LogAuthorityInfo;
use crate::SmartAccountEvent;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RemoveSpendingLimitArgs {
    /// Memo is used for indexing only.
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct RemoveSpendingLimitAsAuthority<'info> {
    #[account(
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    /// Settings `settings_authority` that must authorize the configuration change.
    pub settings_authority: Signer<'info>,

    #[account(mut, close = rent_collector)]
    pub spending_limit: Account<'info, SpendingLimit>,

    /// This is usually the same as `settings_authority`, but can be a different account if needed.
    /// CHECK: can be any account.
    #[account(mut)]
    pub rent_collector: AccountInfo<'info>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl RemoveSpendingLimitAsAuthority<'_> {
    fn validate(&self) -> Result<()> {
        // settings_authority
        require_keys_eq!(
            self.settings_authority.key(),
            self.settings.settings_authority,
            SmartAccountError::Unauthorized
        );

        // `spending_limit`
        require_keys_eq!(
            self.spending_limit.settings,
            self.settings.key(),
            SmartAccountError::InvalidAccount
        );

        Ok(())
    }

    /// Remove the spending limit from the controlled smart account.
    /// NOTE: This instruction must be called only by the `settings_authority` if one is set (Controlled Smart Account).
    ///       Uncontrolled Smart Accounts should use `create_settings_transaction` instead.
    #[access_control(ctx.accounts.validate())]
    pub fn remove_spending_limit(ctx: Context<Self>, _args: RemoveSpendingLimitArgs) -> Result<()> {
        let settings = &ctx.accounts.settings;
        let spending_limit = &ctx.accounts.spending_limit;
        // Log the event
        let event = AuthoritySettingsEvent {
            settings: Settings::try_from_slice(&settings.try_to_vec()?)?,
            settings_pubkey: ctx.accounts.settings.key(),
            authority: ctx.accounts.settings_authority.key(),
            change: SettingsAction::RemoveSpendingLimit {
                spending_limit: spending_limit.key(),
            },
        };
        let log_authority_info = LogAuthorityInfo {
            authority: ctx.accounts.settings.to_account_info(),
            authority_seeds: get_settings_signer_seeds(settings.seed),
            bump: settings.bump,
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::AuthoritySettingsEvent(event).log(&log_authority_info)?;
        Ok(())
    }
}
