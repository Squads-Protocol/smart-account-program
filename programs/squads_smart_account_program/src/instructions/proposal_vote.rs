use anchor_lang::prelude::*;

use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct VoteOnProposalArgs {
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct VoteOnProposal<'info> {
    #[account(
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION,
            &proposal.transaction_index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        bump = proposal.bump,
    )]
    pub proposal: Account<'info, Proposal>,

    // Only required for cancelling a proposal.
    pub system_program: Option<Program<'info, System>>,
}

impl VoteOnProposal<'_> {
    fn validate(&self, ctx: &Context<Self>, vote: Vote) -> Result<()> {
        let Self {
            consensus_account,
            proposal,
            signer,
            ..
        } = self;

        // Check if the consensus account is active
        consensus_account.is_active(&ctx.remaining_accounts)?;

        // signer
        require!(
            consensus_account.is_signer(signer.key()).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            consensus_account.signer_has_permission(signer.key(), Permission::Vote),
            SmartAccountError::Unauthorized
        );

        // proposal
        match vote {
            Vote::Approve | Vote::Reject => {
                require!(
                    matches!(proposal.status, ProposalStatus::Active { .. }),
                    SmartAccountError::InvalidProposalStatus
                );
                // CANNOT approve or reject a stale proposal
                require!(
                    proposal.transaction_index > consensus_account.stale_transaction_index(),
                    SmartAccountError::StaleProposal
                );
            }
            Vote::Cancel => {
                require!(
                    matches!(proposal.status, ProposalStatus::Approved { .. }),
                    SmartAccountError::InvalidProposalStatus
                );
                // CAN cancel a stale proposal.
            }
        }

        Ok(())
    }

    /// Approve a smart account proposal on behalf of the `signer`.
    /// The proposal must be `Active`.
    #[access_control(ctx.accounts.validate(&ctx, Vote::Approve))]
    pub fn approve_proposal(ctx: Context<Self>, _args: VoteOnProposalArgs) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let proposal = &mut ctx.accounts.proposal;
        let signer = &mut ctx.accounts.signer;

        proposal.approve(signer.key(), usize::from(consensus_account.threshold()))?;

        Ok(())
    }

    /// Reject a smart account proposal on behalf of the `signer`.
    /// The proposal must be `Active`.
    #[access_control(ctx.accounts.validate(&ctx, Vote::Reject))]
    pub fn reject_proposal(ctx: Context<Self>, _args: VoteOnProposalArgs) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let proposal = &mut ctx.accounts.proposal;
        let signer = &mut ctx.accounts.signer;

        let cutoff = consensus_account.cutoff();

        proposal.reject(signer.key(), cutoff)?;

        Ok(())
    }

    /// Cancel a smart account proposal on behalf of the `signer`.
    /// The proposal must be `Approved`.
    #[access_control(ctx.accounts.validate(&ctx, Vote::Cancel))]
    pub fn cancel_proposal(ctx: Context<Self>, _args: VoteOnProposalArgs) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let proposal = &mut ctx.accounts.proposal;
        let signer = &mut ctx.accounts.signer;
        let system_program = &ctx
            .accounts
            .system_program
            .as_ref()
            .ok_or(SmartAccountError::MissingAccount)?;

        proposal
            .cancelled
            .retain(|k| consensus_account.is_signer(*k).is_some());

        proposal.cancel(signer.key(), usize::from(consensus_account.threshold()))?;

        Proposal::realloc_if_needed(
            proposal.to_account_info().clone(),
            consensus_account.signers_len(),
            Some(signer.to_account_info().clone()),
            Some(system_program.to_account_info().clone()),
        )?;

        Ok(())
    }
}

pub enum Vote {
    Approve,
    Reject,
    Cancel,
}
