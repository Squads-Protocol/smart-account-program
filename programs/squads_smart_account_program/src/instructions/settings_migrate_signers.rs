use anchor_lang::prelude::*;

use crate::{
    errors::*,
    program::SquadsSmartAccountProgram,
    state::*,
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MigrateSignersArgs {
    /// Optional memo for indexing
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct SettingsMigrateSigners<'info> {
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    /// The settings authority must sign to migrate signers
    pub settings_authority: Signer<'info>,

    /// Payer for any required account reallocation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// System program for reallocation
    pub system_program: Program<'info, System>,

    #[allow(unused)]
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl<'info> SettingsMigrateSigners<'info> {
    fn validate(&self) -> Result<()> {
        // Only the settings authority can migrate signers
        require_keys_eq!(
            self.settings.settings_authority,
            self.settings_authority.key(),
            SmartAccountError::Unauthorized
        );

        // Only V1 signers can be migrated
        require!(
            self.settings.signers.version() == SIGNERS_VERSION_V1,
            SmartAccountError::AlreadyMigrated
        );

        Ok(())
    }

    /// Migrate signers from V1 format to V2 format.
    ///
    /// This instruction converts all V1 (Legacy) signers to V2 format,
    /// enabling support for external signers (P256/WebAuthn, secp256k1, Ed25519).
    ///
    /// After migration, the account may require reallocation as V2 signers
    /// have a slightly different serialization format.
    ///
    /// Requirements:
    /// - Must be called by the settings authority
    /// - Settings must currently use V1 signers
    #[access_control(ctx.accounts.validate())]
    pub fn settings_migrate_signers(
        ctx: Context<SettingsMigrateSigners<'info>>,
        _args: MigrateSignersArgs,
    ) -> Result<()> {
        let settings = &mut ctx.accounts.settings;

        // Get the current V1 signers and convert to V2
        settings.signers = Settings::migrate_signers_wrapper(&settings.signers);

        // Reallocate if needed (V2 format may have different size)
        Settings::realloc_if_needed_for_wrapper(
            settings.to_account_info(),
            &settings.signers,
            Some(ctx.accounts.payer.to_account_info()),
            Some(ctx.accounts.system_program.to_account_info()),
        )?;

        // Emit event
        emit!(SignersMigratedEvent {
            settings: settings.key(),
            signers_count: settings.signers.len() as u16,
        });

        Ok(())
    }
}

#[event]
pub struct SignersMigratedEvent {
    pub settings: Pubkey,
    pub signers_count: u16,
}
