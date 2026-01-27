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
pub struct RemoveSessionKeyArgs {
    /// The key (for Native) or key_id (for External) of the parent signer
    pub parent_signer_key: Pubkey,
    /// Optional client data params for WebAuthn verification
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
    /// Optional memo for indexing
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct RemoveSessionKey<'info> {
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    /// The parent signer's authority that must authorize the session key removal.
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

impl RemoveSessionKey<'_> {
    fn validate(&self) -> Result<()> {
        // Ensure settings are in V2 format
        require!(
            self.settings.signers.version() == SIGNERS_VERSION_V2,
            SmartAccountError::MustMigrateToV2
        );

        Ok(())
    }

    /// Remove a session key from an existing V2 signer.
    ///
    /// This instruction revokes the temporary signing authority that was delegated to a
    /// native Solana key via a session key. After removal, the session key can no longer
    /// be used to sign on behalf of the parent signer.
    ///
    /// Requirements:
    /// - Settings must be in V2 format
    /// - Parent signer must exist and be an external signer
    /// - Parent signer must provide valid signature (native or external)
    #[access_control(ctx.accounts.validate())]
    pub fn remove_session_key<'info>(
        ctx: Context<'_, '_, 'info, 'info, RemoveSessionKey<'info>>,
        args: RemoveSessionKeyArgs,
    ) -> Result<()> {
        let RemoveSessionKeyArgs {
            parent_signer_key,
            client_data_params,
            ..
        } = args;

        let settings = &mut ctx.accounts.settings;

        // Construct expected message for signature verification
        let mut message_data = Vec::with_capacity(80);
        message_data.extend_from_slice(b"remove_session_key");
        message_data.extend_from_slice(&settings.key().to_bytes());
        message_data.extend_from_slice(&parent_signer_key.to_bytes());

        // Verify parent signer (native or external)
        let verified_key = verify_v2_context(
            settings,
            parent_signer_key,
            ctx.remaining_accounts,
            &message_data,
            client_data_params.as_ref(),
        )?;

        // Find the parent signer in settings and clear session key
        let mut signer_found = false;
        let mut signer_is_native = false;

        if let Some(parent_signer) = settings.signers.find_mut(&verified_key) {
            signer_found = true;
            signer_is_native = parent_signer.is_native();

            // Clear session key
            parent_signer.clear_session_key()?;
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
            change: SettingsAction::ClearSessionKey {
                signer_key: parent_signer_key,
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
