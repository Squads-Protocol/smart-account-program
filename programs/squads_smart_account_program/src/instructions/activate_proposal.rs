use anchor_lang::prelude::*;

use crate::errors::*;
use crate::state::*;

#[derive(Accounts)]
pub struct ActivateProposal<'info> {
    #[account(
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    #[account(mut)]
    pub signer: Signer<'info>,

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
    fn validate(&self) -> Result<()> {
        let Self {
            settings,
            proposal,
            signer,
            ..
        } = self;

        // `signer`
        require!(
            settings.is_signer(signer.key()).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            // We consider this action a part of the proposal initiation.
            settings.signer_has_permission(signer.key(), Permission::Initiate),
            SmartAccountError::Unauthorized
        );

        // `proposal`
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
    #[access_control(ctx.accounts.validate())]
    pub fn activate_proposal(ctx: Context<Self>) -> Result<()> {
        ctx.accounts.proposal.status = ProposalStatus::Active {
            timestamp: Clock::get()?.unix_timestamp,
        };

        Ok(())
    }
}
