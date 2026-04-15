use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::events::*;
use crate::interface::consensus::ConsensusAccount;
use crate::program::SquadsSmartAccountProgram;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::create_vote_message;

use crate::state::*;
use crate::instructions::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct VoteOnProposalArgs {
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct VoteOnProposal<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    pub signer: ResolvedSigner<'info>,

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
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl VoteOnProposal<'_> {
    fn validate(
        &mut self,
        vote: Vote,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        let Self {
            consensus_account,
            proposal,
            signer,
            ..
        } = self;

        // Check if the consensus account is active
        consensus_account.is_active(&remaining_accounts)?;

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

        // Build the message for this vote operation
        let message = create_vote_message(
            &proposal.key(),
            vote.to_u8(),
            proposal.transaction_index,
        );

        // Resolve and verify signer (native, session key, or external) and check Vote permission
        signer.verify(
            &mut **consensus_account,
            remaining_accounts,
            message,
            extra_verification_data.as_ref(),
            Some(Permission::Vote),
        )?;

        Ok(())
    }

    /// Approve a smart account proposal on behalf of the `signer`.
    /// The proposal must be `Active`.
    #[access_control(ctx.accounts.validate(Vote::Approve, &ctx.remaining_accounts, None))]
    pub fn approve_proposal(ctx: Context<Self>, args: VoteOnProposalArgs) -> Result<()> {
        Self::approve_proposal_inner(ctx, args)
    }

    /// Approve a smart account proposal with V2 signer support.
    #[access_control(ctx.accounts.validate(Vote::Approve, &ctx.remaining_accounts, extra_verification_data))]
    pub fn approve_proposal_v2(ctx: Context<Self>, args: VoteOnProposalArgs, extra_verification_data: Option<ExtraVerificationData>) -> Result<()> {
        Self::approve_proposal_inner(ctx, args)
    }

    fn approve_proposal_inner(ctx: Context<Self>, args: VoteOnProposalArgs) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;

        let proposal = &mut ctx.accounts.proposal;
        let signer = &ctx.accounts.signer;

        // Use resolved key (parent signer key for session keys) to prevent double-voting
        let resolved_key = signer.resolved_key()?;
        proposal.approve(resolved_key, usize::from(consensus_account.threshold()))?;

        // Log the vote event with proposal state
        let vote_event = ProposalEvent {
            event_type: ProposalEventType::Approve,
            consensus_account: consensus_account.key(),
            consensus_account_type: consensus_account.account_type(),
            proposal_pubkey: proposal.key(),
            transaction_index: proposal.transaction_index,
            signer: Some(resolved_key),
            memo: args.memo,
            proposal: Some(Proposal::try_from_slice(&proposal.try_to_vec()?)?),
        };
        let log_authority_info = LogAuthorityInfo {
            authority: consensus_account.to_account_info(),
            authority_seeds: consensus_account.get_signer_seeds(),
            bump: consensus_account.bump(),
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::ProposalEvent(vote_event).log(&log_authority_info)?;

        Ok(())
    }

    /// Reject a smart account proposal on behalf of the `signer`.
    /// The proposal must be `Active`.
    #[access_control(ctx.accounts.validate(Vote::Reject, &ctx.remaining_accounts, None))]
    pub fn reject_proposal(ctx: Context<Self>, args: VoteOnProposalArgs) -> Result<()> {
        Self::reject_proposal_inner(ctx, args)
    }

    /// Reject a smart account proposal with V2 signer support.
    #[access_control(ctx.accounts.validate(Vote::Reject, &ctx.remaining_accounts, extra_verification_data))]
    pub fn reject_proposal_v2(ctx: Context<Self>, args: VoteOnProposalArgs, extra_verification_data: Option<ExtraVerificationData>) -> Result<()> {
        Self::reject_proposal_inner(ctx, args)
    }

    fn reject_proposal_inner(ctx: Context<Self>, args: VoteOnProposalArgs) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let proposal = &mut ctx.accounts.proposal;
        let signer = &ctx.accounts.signer;

        let cutoff = consensus_account.cutoff();

        // Use resolved key (parent signer key for session keys) to prevent double-voting
        let resolved_key = signer.resolved_key()?;
        proposal.reject(resolved_key, cutoff)?;

        // Log the vote event with proposal state
        let vote_event = ProposalEvent {
            event_type: ProposalEventType::Reject,
            consensus_account: consensus_account.key(),
            consensus_account_type: consensus_account.account_type(),
            proposal_pubkey: proposal.key(),
            transaction_index: proposal.transaction_index,
            signer: Some(resolved_key),
            memo: args.memo,
            proposal: Some(proposal.clone().into_inner()),
        };
        let log_authority_info = LogAuthorityInfo {
            authority: consensus_account.to_account_info(),
            authority_seeds: consensus_account.get_signer_seeds(),
            bump: consensus_account.bump(),
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::ProposalEvent(vote_event).log(&log_authority_info)?;

        Ok(())
    }

    /// Cancel a smart account proposal on behalf of the `signer`.
    /// The proposal must be `Approved`.
    #[access_control(ctx.accounts.validate(Vote::Cancel, &ctx.remaining_accounts, None))]
    pub fn cancel_proposal(ctx: Context<Self>, args: VoteOnProposalArgs) -> Result<()> {
        Self::cancel_proposal_inner(ctx, args)
    }

    /// Cancel a smart account proposal with V2 signer support.
    #[access_control(ctx.accounts.validate(Vote::Cancel, &ctx.remaining_accounts, extra_verification_data))]
    pub fn cancel_proposal_v2(ctx: Context<Self>, args: VoteOnProposalArgs, extra_verification_data: Option<ExtraVerificationData>) -> Result<()> {
        Self::cancel_proposal_inner(ctx, args)
    }

    fn cancel_proposal_inner(ctx: Context<Self>, args: VoteOnProposalArgs) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let proposal = &mut ctx.accounts.proposal;
        let signer = &ctx.accounts.signer;
        let system_program = &ctx
            .accounts
            .system_program
            .as_ref()
            .ok_or(SmartAccountError::MissingAccount)?;

        proposal
            .cancelled
            .retain(|k| consensus_account.is_signer(*k).is_some());

        // Use resolved key (parent signer key for session keys) to prevent double-voting
        let resolved_key = signer.resolved_key()?;
        proposal.cancel(resolved_key, usize::from(consensus_account.threshold()))?;

        Proposal::realloc_if_needed(
            proposal.to_account_info().clone(),
            consensus_account.signers_len(),
            Some(signer.to_account_info().clone()),
            Some(system_program.to_account_info().clone()),
        )?;

        // Log the vote event with proposal state
        let vote_event = ProposalEvent {
            event_type: ProposalEventType::Cancel,
            consensus_account: consensus_account.key(),
            consensus_account_type: consensus_account.account_type(),
            proposal_pubkey: proposal.key(),
            transaction_index: proposal.transaction_index,
            signer: Some(resolved_key),
            memo: args.memo,
            proposal: Some(proposal.clone().into_inner()),
        };
        let log_authority_info = LogAuthorityInfo {
            authority: consensus_account.to_account_info(),
            authority_seeds: consensus_account.get_signer_seeds(),
            bump: consensus_account.bump(),
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::ProposalEvent(vote_event).log(&log_authority_info)?;

        Ok(())
    }
}


pub enum Vote {
    Approve,
    Reject,
    Cancel,
}

impl Vote {
    pub fn to_u8(&self) -> u8 {
        match self {
            Vote::Approve => 0,
            Vote::Reject => 1,
            Vote::Cancel => 2,
        }
    }
}
