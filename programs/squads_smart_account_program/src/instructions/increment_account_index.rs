use anchor_lang::prelude::*;

use crate::{
    errors::SmartAccountError,
    events::{IncrementAccountIndexEvent, LogAuthorityInfo, SmartAccountEvent},
    interface::consensus_trait::Consensus,
    program::SquadsSmartAccountProgram,
    state::{
        get_settings_signer_seeds, Permission, Settings, FREE_ACCOUNT_MAX_INDEX, SEED_PREFIX,
        SEED_SETTINGS,
    },
    state::signer_v2::ExtraVerificationData,
    state::signer_v2::precompile::{split_instructions_sysvar, verify_precompile_signers},
    utils::context_validation::verify_external_signer_via_syscall,
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

    /// The signer (native or external).
    /// CHECK: Validated as a signer of the settings account.
    pub signer: AccountInfo<'info>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl IncrementAccountIndexV2<'_> {
    fn validate(
        &mut self,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: &Option<ExtraVerificationData>,
    ) -> Result<()> {
        let settings = &self.settings;
        let signer_key = *self.signer.key;

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

        // Verify signer: native or external
        if signer.is_external() {
            let evd = extra_verification_data
                .as_ref()
                .ok_or(SmartAccountError::MissingExtraVerificationData)?;

            // Build a domain-specific message for this operation
            let mut message = anchor_lang::solana_program::hash::Hasher::default();
            message.hash(b"increment_account_index_v2");
            message.hash(settings.key().as_ref());
            message.hash(signer_key.as_ref());

            let (sysvar_opt, _) = split_instructions_sysvar(remaining_accounts);

            let (counter_update, next_nonce) = if evd.is_precompile() {
                let sysvar = sysvar_opt
                    .ok_or(SmartAccountError::MissingPrecompileInstruction)?;
                let results = verify_precompile_signers(
                    sysvar,
                    &[signer.clone()],
                    &[evd.clone()],
                    &message,
                )?;
                results
                    .into_iter()
                    .next()
                    .ok_or_else(|| error!(SmartAccountError::MissingPrecompileInstruction))?
            } else {
                verify_external_signer_via_syscall(&signer, &message, evd)?
            };

            // Apply counter update if needed (WebAuthn)
            if let Some(new_counter) = counter_update {
                self.settings
                    .signers
                    .update_signer_counter(&signer_key, new_counter)?;
            }

            // Apply nonce update
            self.settings
                .signers
                .update_signer_nonce(&signer_key, next_nonce)?;
        } else {
            // Native signer must be a native tx signer
            require!(self.signer.is_signer, SmartAccountError::MissingSignature);
        }

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
