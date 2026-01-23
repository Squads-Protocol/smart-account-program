use anchor_lang::prelude::*;

use crate::{
    consensus_trait::Consensus,
    errors::*,
    program::SquadsSmartAccountProgram,
    state::*,
    AuthoritySettingsEvent, LogAuthorityInfo, SmartAccountEvent,
};

/// Arguments for removing a V2 signer (Native or External) from the smart account.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RemoveSignerV2Args {
    /// The key (for Native) or key_id (for External) of the signer to remove
    pub key: Pubkey,
    /// Optional memo for indexing
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct SettingsRemoveSignerV2<'info> {
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    /// Settings `settings_authority` that must authorize the configuration change.
    pub settings_authority: Signer<'info>,

    /// Payer for rent return (optional, in case account shrinks)
    #[account(mut)]
    pub rent_payer: Option<Signer<'info>>,

    /// System program
    pub system_program: Option<Program<'info, System>>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl<'info> SettingsRemoveSignerV2<'info> {
    fn validate(&self) -> Result<()> {
        require_keys_eq!(
            self.settings_authority.key(),
            self.settings.settings_authority,
            SmartAccountError::Unauthorized
        );

        // Settings must be migrated to V2 format before using this instruction
        require!(
            self.settings.signers.version() == SIGNERS_VERSION_V2,
            SmartAccountError::MustMigrateToV2
        );

        Ok(())
    }

    /// Remove a V2 signer (Native or External) from the smart account.
    ///
    /// This is the V2 version of `remove_signer_as_authority` that works with
    /// `SmartAccountSigner` instead of `LegacySmartAccountSigner`.
    ///
    /// Requirements:
    /// - Must be called by the `settings_authority` (Controlled Smart Account)
    /// - Settings must be migrated to V2 signer format
    /// - Cannot remove the last signer
    ///
    /// For uncontrolled Smart Accounts, use `create_settings_transaction` with
    /// `SettingsAction::RemoveSignerV2` instead.
    #[access_control(ctx.accounts.validate())]
    pub fn remove_signer_v2(ctx: Context<SettingsRemoveSignerV2<'info>>, args: RemoveSignerV2Args) -> Result<()> {
        let settings = &mut ctx.accounts.settings;

        // Cannot remove the last signer
        require!(
            settings.signers.len() > 1,
            SmartAccountError::RemoveLastSigner
        );

        // Remove the signer
        settings.remove_signer(args.key)?;

        settings.invalidate_prior_transactions();

        settings.invariant()?;

        // Log the event
        let event = AuthoritySettingsEvent {
            settings: Settings::try_from_slice(&settings.try_to_vec()?)?,
            settings_pubkey: settings.key(),
            authority: ctx.accounts.settings_authority.key(),
            change: SettingsAction::RemoveSignerV2 {
                old_signer: args.key,
            },
        };
        let log_authority_info = LogAuthorityInfo {
            authority: settings.to_account_info(),
            authority_seeds: get_settings_signer_seeds(settings.seed),
            bump: settings.bump,
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::AuthoritySettingsEvent(event).log(&log_authority_info)?;

        Ok(())
    }
}
