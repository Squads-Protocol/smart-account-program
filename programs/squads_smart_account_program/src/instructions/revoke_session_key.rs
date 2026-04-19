use anchor_lang::prelude::*;

use crate::{errors::*, program::SquadsSmartAccountProgram, state::*};
use crate::consensus_trait::Consensus;
use crate::interface::consensus::ConsensusAccount;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::{
    create_revoke_session_key_message, split_instructions_sysvar, verify_precompile_signers,
};
use crate::utils::context_validation::verify_external_signer_via_syscall;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RevokeSessionKeyArgs {
    /// The key_id of the external signer whose session key to revoke
    pub signer_key: Pubkey,
}

#[derive(Accounts)]
pub struct RevokeSessionKey<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    /// The authority revoking the session key. Can be either:
    /// 1. The external signer (verified via precompile/syscall)
    /// 2. The current session key holder (verified via is_signer)
    /// CHECK: Validated in the handler logic.
    pub authority: AccountInfo<'info>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl RevokeSessionKey<'_> {
    /// Revoke a session key from an external signer.
    ///
    /// Authorization: either the external signer (via precompile/syscall)
    /// or the current active session key holder (via native signature).
    /// Works on both Settings and Policy consensus accounts.
    pub fn revoke_session_key(
        ctx: Context<Self>,
        args: RevokeSessionKeyArgs,
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let authority_key = *ctx.accounts.authority.key;
        let now = Clock::get()?.unix_timestamp as u64;

        // Look up the target signer — must exist and be external
        let signer = consensus_account
            .signers()
            .find(&args.signer_key)
            .ok_or(SmartAccountError::NotASigner)?;

        require!(
            signer.is_external(),
            SmartAccountError::InvalidSignerType
        );

        if authority_key == args.signer_key {
            // Path 1: External signer is revoking their own session key
            let evd = extra_verification_data
                .ok_or(SmartAccountError::MissingExtraVerificationData)?;

            let message = create_revoke_session_key_message(
                &consensus_account.key(),
                &args.signer_key,
            );

            let (sysvar_opt, _) = split_instructions_sysvar(&ctx.remaining_accounts);

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
                verify_external_signer_via_syscall(&signer, &message, &evd)?
            };

            // Apply counter update if needed (WebAuthn)
            if let Some(new_counter) = counter_update {
                consensus_account
                    .signers_mut()
                    .update_signer_counter(&args.signer_key, new_counter)?;
            }

            // Apply nonce update
            consensus_account
                .signers_mut()
                .update_signer_nonce(&args.signer_key, next_nonce)?;
        } else {
            // Path 2: Session key holder is revoking their own delegation
            require!(
                ctx.accounts.authority.is_signer,
                SmartAccountError::MissingSignature
            );

            // Verify the authority IS the current session key and it's active
            require!(
                signer.is_valid_session_key(&authority_key, now),
                SmartAccountError::InvalidSessionKey
            );
        }

        // Clear the session key
        let signer_mut = consensus_account
            .signers_mut()
            .find_mut(&args.signer_key)
            .ok_or(SmartAccountError::NotASigner)?;

        signer_mut.clear_session_key()?;

        // Re-validate consensus invariant after mutation
        consensus_account.invariant()?;

        Ok(())
    }
}
