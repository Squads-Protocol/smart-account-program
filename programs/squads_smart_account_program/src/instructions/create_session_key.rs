use anchor_lang::prelude::*;

use crate::{errors::*, program::SquadsSmartAccountProgram, state::*};
use crate::consensus_trait::Consensus;
use crate::interface::consensus::ConsensusAccount;
use crate::instructions::*;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::{
    create_session_key_message, split_instructions_sysvar, verify_precompile_signers,
};
use crate::utils::context_validation::verify_external_signer_via_syscall;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateSessionKeyArgs {
    /// The new session key pubkey (a native Solana keypair)
    pub session_key: Pubkey,
    /// Session key expiration timestamp (Unix seconds)
    pub session_key_expiration: u64,
}

#[derive(Accounts)]
pub struct CreateSessionKey<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    pub signer: ResolvedSigner<'info>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl CreateSessionKey<'_> {
    fn validate(
        &mut self,
        args: &CreateSessionKeyArgs,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: &Option<ExtraVerificationData>,
    ) -> Result<()> {
        let extra_verification_data = extra_verification_data
            .as_ref()
            .ok_or(SmartAccountError::MissingExtraVerificationData)?;
        let consensus_account = &self.consensus_account;
        let signer_key = self.signer.key();

        // Look up the signer — must exist and be an external signer
        let signer = consensus_account
            .signers()
            .find(&signer_key)
            .ok_or(SmartAccountError::NotASigner)?;

        require!(
            signer.is_external(),
            SmartAccountError::InvalidSignerType
        );

        // Build domain-specific message for session key creation
        let message = create_session_key_message(
            &consensus_account.key(),
            &signer_key,
            &args.session_key,
            args.session_key_expiration,
        );

        // Verify external signer via precompile or syscall
        let (sysvar_opt, _) = split_instructions_sysvar(remaining_accounts);

        let (counter_update, next_nonce) = if extra_verification_data.is_precompile() {
            let sysvar = sysvar_opt
                .ok_or(SmartAccountError::MissingPrecompileInstruction)?;
            let results = verify_precompile_signers(
                sysvar,
                &[signer.clone()],
                &[extra_verification_data.clone()],
                &message,
            )?;
            results
                .into_iter()
                .next()
                .ok_or_else(|| error!(SmartAccountError::MissingPrecompileInstruction))?
        } else {
            verify_external_signer_via_syscall(&signer, &message, extra_verification_data)?
        };

        // Apply counter update if needed (WebAuthn)
        if let Some(new_counter) = counter_update {
            self.consensus_account
                .signers_mut()
                .update_signer_counter(&signer_key, new_counter)?;
        }

        // Apply nonce update
        self.consensus_account
            .signers_mut()
            .update_signer_nonce(&signer_key, next_nonce)?;

        Ok(())
    }

    /// Create a session key for an external signer.
    ///
    /// Only external signers (P256Webauthn, Secp256k1, Ed25519External, P256Native) support session keys.
    /// The external signer must prove ownership via precompile or syscall verification.
    /// Works on both Settings and Policy consensus accounts.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, &extra_verification_data))]
    pub fn create_session_key(
        ctx: Context<Self>,
        args: CreateSessionKeyArgs,
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let signer_key = ctx.accounts.signer.key();
        let now = Clock::get()?.unix_timestamp as u64;

        // Prevent session key from colliding with an existing signer key.
        // If a signer is later removed, the session key could inherit an unexpected role.
        require!(
            consensus_account.signers().find(&args.session_key).is_none(),
            SmartAccountError::InvalidSessionKey
        );

        // Prevent session key from colliding with another signer's session key.
        // Without this, find_signer_by_session_key returns the first match,
        // silently shadowing the second signer's session key.
        require!(
            !consensus_account.signers().has_session_key_assigned(&args.session_key),
            SmartAccountError::InvalidSessionKey
        );

        // Get mutable reference to the signer and set session key
        let signer = consensus_account
            .signers_mut()
            .find_mut(&signer_key)
            .ok_or(SmartAccountError::NotASigner)?;

        signer.set_session_key(args.session_key, args.session_key_expiration, now)?;

        // Re-validate consensus invariant after mutation
        consensus_account.invariant()?;

        Ok(())
    }
}
