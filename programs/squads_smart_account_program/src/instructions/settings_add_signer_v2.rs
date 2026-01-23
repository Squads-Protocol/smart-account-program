use anchor_lang::prelude::*;

use crate::{
    consensus_trait::Consensus,
    errors::SmartAccountError,
    program::SquadsSmartAccountProgram,
    state::*,
    AuthoritySettingsEvent, LogAuthorityInfo, SmartAccountEvent,
};

/// Arguments for adding a V2 signer (Native or External) to the smart account.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddSignerV2Args {
    /// The signer type (0=Native, 1=P256Webauthn, 2=Secp256k1, 3=Ed25519External)
    pub signer_type: u8,
    /// For Native: the signer's pubkey (used directly as key).
    /// For external signers: ignored - key_id is derived from the public key in signer_data.
    pub key: Pubkey,
    /// Permissions for the signer
    pub permissions: Permissions,
    /// Signer-specific data:
    /// - Native: empty (0 bytes)
    /// - P256Webauthn: 74 bytes (compressed_pubkey(33) + rp_id_len(1) + rp_id(32) + counter(8))
    ///   Note: rp_id_hash is derived from rp_id, not provided by caller
    /// - Secp256k1: 85 bytes (uncompressed_pubkey(64) + eth_address(20) + has_eth_address(1))
    /// - Ed25519External: 32 bytes (external_pubkey)
    pub signer_data: Vec<u8>,
    /// Optional memo for indexing
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct SettingsAddSignerV2<'info> {
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    /// Settings `settings_authority` that must authorize the configuration change.
    pub settings_authority: Signer<'info>,

    /// Payer for any required account reallocation
    #[account(mut)]
    pub rent_payer: Option<Signer<'info>>,

    /// System program for reallocation
    pub system_program: Option<Program<'info, System>>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl<'info> SettingsAddSignerV2<'info> {
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

    /// Parse and validate signer data from args to create a SmartAccountSigner.
    ///
    /// For external signers, the key_id is derived deterministically from the public key
    /// to ensure cryptographic binding between the key_id and the actual public key.
    fn parse_signer(args: &AddSignerV2Args) -> Result<SmartAccountSigner> {
        SmartAccountSigner::from_raw_data(
            args.signer_type,
            args.key,
            args.permissions,
            &args.signer_data,
        )
    }

    /// Add a V2 signer (Native or External) to the smart account.
    ///
    /// This is the V2 version of `add_signer_as_authority` that works with
    /// `SmartAccountSigner` instead of `LegacySmartAccountSigner`.
    ///
    /// Requirements:
    /// - Must be called by the `settings_authority` (Controlled Smart Account)
    /// - Settings must be migrated to V2 signer format
    ///
    /// For uncontrolled Smart Accounts, use `create_settings_transaction` with
    /// `SettingsAction::AddSignerV2` instead.
    #[access_control(ctx.accounts.validate())]
    pub fn add_signer_v2(ctx: Context<SettingsAddSignerV2<'info>>, args: AddSignerV2Args) -> Result<()> {
        let new_signer = Self::parse_signer(&args)?;
        let settings = &mut ctx.accounts.settings;

        settings.add_signer_v2_checked(&new_signer)?;

        // Reallocate if needed (V2 format may require more space)
        let new_size = Settings::size_for_wrapper(&settings.signers);
        let current_size = settings.to_account_info().data_len();

        if new_size > current_size {
            crate::utils::realloc(
                &settings.to_account_info(),
                new_size,
                ctx.accounts
                    .rent_payer
                    .as_ref()
                    .map(ToAccountInfo::to_account_info),
                ctx.accounts
                    .system_program
                    .as_ref()
                    .map(ToAccountInfo::to_account_info),
            )?;
        }

        settings.invariant()?;

        // Log the event
        let event = AuthoritySettingsEvent {
            settings: Settings::try_from_slice(&settings.try_to_vec()?)?,
            settings_pubkey: settings.key(),
            authority: ctx.accounts.settings_authority.key(),
            change: SettingsAction::AddSignerV2 {
                new_signer,
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
