use anchor_lang::prelude::*;

use crate::{
    consensus_trait::Consensus,
    errors::SmartAccountError,
    events::{AuthoritySettingsEvent, LogAuthorityInfo, SmartAccountEvent},
    get_settings_signer_seeds,
    program::SquadsSmartAccountProgram,
    state::*,
    utils::verify_v2_context,
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddSessionKeyArgs {
    /// The key (for Native) or key_id (for External) of the parent signer
    pub parent_signer_key: Pubkey,
    /// The session key pubkey (native Solana key that can sign temporarily)
    pub session_key: Pubkey,
    /// Session key expiration timestamp (Unix seconds)
    pub expiration: u64,
    /// Optional client data params for WebAuthn verification
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
    /// Optional memo for indexing
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct AddSessionKey<'info> {
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    /// The parent signer's authority that must authorize the session key addition.
    /// For native signers, this is a transaction signer.
    /// For external signers, their signature is verified via precompile introspection.
    pub parent_signer: Signer<'info>,

    /// The account that will be charged or credited in case the settings account needs to reallocate space.
    /// This is usually the same as `parent_signer`, but can be a different account if needed.
    #[account(mut)]
    pub rent_payer: Option<Signer<'info>>,

    /// We might need it in case reallocation is needed.
    pub system_program: Option<Program<'info, System>>,
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl AddSessionKey<'_> {
    fn validate(&self) -> Result<()> {
        // Ensure settings are in V2 format
        require!(
            self.settings.signers.version() == SIGNERS_VERSION_V2,
            SmartAccountError::MustMigrateToV2
        );

        Ok(())
    }

    /// Add a session key to an existing V2 signer.
    ///
    /// This instruction allows an external signer (P256WebAuthn, Secp256k1, Ed25519External) to
    /// delegate temporary signing authority to a native Solana key for a limited time period.
    ///
    /// Requirements:
    /// - Settings must be in V2 format
    /// - Parent signer must exist and be an external signer
    /// - Session key must not be the default pubkey
    /// - Expiration must be in the future (> current_timestamp)
    /// - Expiration must not exceed 3 months from now
    /// - Parent signer must provide valid signature (native or external)
    #[access_control(ctx.accounts.validate())]
    pub fn add_session_key<'info>(
        ctx: Context<'_, '_, 'info, 'info, AddSessionKey<'info>>,
        args: AddSessionKeyArgs,
    ) -> Result<()> {
        let AddSessionKeyArgs {
            parent_signer_key,
            session_key,
            expiration,
            client_data_params,
            ..
        } = args;

        let settings = &mut ctx.accounts.settings;
        let current_timestamp = Clock::get()?.unix_timestamp as u64;

        // Construct expected message for signature verification
        let mut message_data = Vec::with_capacity(100);
        message_data.extend_from_slice(b"add_session_key");
        message_data.extend_from_slice(&settings.key().to_bytes());
        message_data.extend_from_slice(&parent_signer_key.to_bytes());
        message_data.extend_from_slice(&session_key.to_bytes());
        message_data.extend_from_slice(&expiration.to_le_bytes());

        // Verify parent signer (native or external)
        let verified_key = verify_v2_context(
            settings,
            parent_signer_key,
            ctx.remaining_accounts,
            &message_data,
            client_data_params.as_ref(),
        )?;

        // Find the parent signer in settings and add session key
        let mut signer_found = false;
        let mut signer_is_native = false;

        if let Some(parent_signer) = settings.signers.find_mut(&verified_key) {
            signer_found = true;
            signer_is_native = parent_signer.is_native();

            // Set session key with validation (will check expiration, limits, etc.)
            parent_signer.set_session_key(session_key, expiration, current_timestamp)?;
        }

        require!(signer_found, SmartAccountError::NotASigner);
        require!(!signer_is_native, SmartAccountError::InvalidSignerType);

        // Reallocate if needed
        Settings::realloc_if_needed_for_wrapper(
            settings.to_account_info(),
            &settings.signers,
            ctx.accounts
                .rent_payer
                .as_ref()
                .map(ToAccountInfo::to_account_info),
            ctx.accounts
                .system_program
                .as_ref()
                .map(ToAccountInfo::to_account_info),
        )?;

        settings.invariant()?;

        // Log the event
        let event = AuthoritySettingsEvent {
            settings: Settings::try_from_slice(&settings.try_to_vec()?)?,
            settings_pubkey: settings.key(),
            authority: ctx.accounts.parent_signer.key(),
            change: SettingsAction::SetSessionKey {
                signer_key: parent_signer_key,
                session_key,
                expiration,
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
