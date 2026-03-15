use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::state::*;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::create_proposal_activate_message;

#[derive(Accounts)]
pub struct ActivateProposal<'info> {
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    // #[account(mut)]: @orion: does this need to be mutable?
    /// CHECK: This account will be checked in settings.verify_signer
    pub signer: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &proposal.transaction_index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        bump = proposal.bump,
    )]
    pub proposal: Account<'info, Proposal>,
}

impl ActivateProposal<'_> {
    fn validate(&mut self, remaining_accounts: &[AccountInfo], extra_verification_data: Option<ExtraVerificationData>) -> Result<()> {
        let Self {
            settings,
            proposal,
            signer,
            ..
        } = self;

        let message = create_proposal_activate_message(&proposal.key(), proposal.transaction_index);

        // Verify both the legitimacy of the signer and that it has the right permissions
        settings.verify_signer(
            signer,
            remaining_accounts,
            message,
            extra_verification_data.as_ref(),
            Some(Permission::Initiate),
        )?;

        // Proposal must be in draft status and not stale
        require!(
            matches!(proposal.status, ProposalStatus::Draft { .. }),
            SmartAccountError::InvalidProposalStatus
        );
        require!(
            proposal.transaction_index > settings.stale_transaction_index,
            SmartAccountError::StaleProposal
        );

        Ok(())
    }

    /// Update status of a multisig proposal from `Draft` to `Active`.
    #[access_control(ctx.accounts.validate(&ctx.remaining_accounts, None))]
    pub fn activate_proposal(ctx: Context<Self>) -> Result<()> {
        ctx.accounts.proposal.status = ProposalStatus::Active {
            timestamp: Clock::get()?.unix_timestamp,
        };

        Ok(())
    }

    /// Update status of a multisig proposal from `Draft` to `Active` with V2 signers.
    #[access_control(ctx.accounts.validate(&ctx.remaining_accounts, extra_verification_data))]
    pub fn activate_proposal_v2(ctx: Context<Self>, extra_verification_data: Option<ExtraVerificationData>) -> Result<()> {
        ctx.accounts.proposal.status = ProposalStatus::Active {
            timestamp: Clock::get()?.unix_timestamp,
        };

        Ok(())
    }
}
