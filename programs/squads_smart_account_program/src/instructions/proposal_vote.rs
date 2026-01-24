use anchor_lang::prelude::*;
use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::events::*;
use crate::interface::consensus::ConsensusAccount;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;
use crate::utils::{create_vote_message, verify_v2_context};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct VoteOnProposalArgs {
    pub memo: Option<String>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct VoteOnProposalV2Args {
    pub voter_key: Pubkey,
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
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
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

#[derive(Accounts)]
pub struct VoteOnProposalV2<'info> {
    #[account(
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

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

    /// Optional payer for reallocation (required for cancel)
    #[account(mut)]
    pub payer: Option<Signer<'info>>,

    /// System program for reallocation
    pub system_program: Option<Program<'info, System>>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl VoteOnProposal<'_> {
    fn validate(&self, ctx: &Context<Self>, vote: Vote) -> Result<()> {
        validate_vote(
            &self.consensus_account,
            &self.proposal,
            self.signer.key(),
            vote,
            &ctx.remaining_accounts,
        )
    }

    /// Approve a smart account proposal on behalf of the `signer`.
    /// The proposal must be `Active`.
    #[access_control(ctx.accounts.validate(&ctx, Vote::Approve))]
    pub fn approve_proposal(ctx: Context<Self>, args: VoteOnProposalArgs) -> Result<()> {
        proposal_vote_inner(
            &mut ctx.accounts.consensus_account,
            &mut ctx.accounts.proposal,
            ctx.accounts.signer.key(),
            Vote::Approve,
            Some(&ctx.accounts.signer),
            ctx.accounts.system_program.as_ref(),
            &ctx.accounts.program,
            args.memo,
        )
    }

    /// Reject a smart account proposal on behalf of the `signer`.
    /// The proposal must be `Active`.
    #[access_control(ctx.accounts.validate(&ctx, Vote::Reject))]
    pub fn reject_proposal(ctx: Context<Self>, args: VoteOnProposalArgs) -> Result<()> {
        proposal_vote_inner(
            &mut ctx.accounts.consensus_account,
            &mut ctx.accounts.proposal,
            ctx.accounts.signer.key(),
            Vote::Reject,
            Some(&ctx.accounts.signer),
            ctx.accounts.system_program.as_ref(),
            &ctx.accounts.program,
            args.memo,
        )
    }

    /// Cancel a smart account proposal on behalf of the `signer`.
    /// The proposal must be `Approved`.
    #[access_control(ctx.accounts.validate(&ctx, Vote::Cancel))]
    pub fn cancel_proposal(ctx: Context<Self>, args: VoteOnProposalArgs) -> Result<()> {
        proposal_vote_inner(
            &mut ctx.accounts.consensus_account,
            &mut ctx.accounts.proposal,
            ctx.accounts.signer.key(),
            Vote::Cancel,
            Some(&ctx.accounts.signer),
            ctx.accounts.system_program.as_ref(),
            &ctx.accounts.program,
            args.memo,
        )
    }
}

impl VoteOnProposalV2<'_> {
    fn validate(&self, ctx: &Context<Self>, vote: Vote, voter_key: Pubkey) -> Result<()> {
        validate_vote(
            &self.consensus_account,
            &self.proposal,
            voter_key,
            vote,
            &ctx.remaining_accounts,
        )
    }

    #[access_control(ctx.accounts.validate(&ctx, Vote::Approve, args.voter_key))]
    pub fn approve_proposal_v2(mut ctx: Context<Self>, args: VoteOnProposalV2Args) -> Result<()> {
        verify_vote_v2(&mut ctx, &args, Vote::Approve)?;

        proposal_vote_inner(
            &mut ctx.accounts.consensus_account,
            &mut ctx.accounts.proposal,
            args.voter_key,
            Vote::Approve,
            ctx.accounts.payer.as_ref(),
            ctx.accounts.system_program.as_ref(),
            &ctx.accounts.program,
            args.memo,
        )
    }

    #[access_control(ctx.accounts.validate(&ctx, Vote::Reject, args.voter_key))]
    pub fn reject_proposal_v2(mut ctx: Context<Self>, args: VoteOnProposalV2Args) -> Result<()> {
        verify_vote_v2(&mut ctx, &args, Vote::Reject)?;

        proposal_vote_inner(
            &mut ctx.accounts.consensus_account,
            &mut ctx.accounts.proposal,
            args.voter_key,
            Vote::Reject,
            ctx.accounts.payer.as_ref(),
            ctx.accounts.system_program.as_ref(),
            &ctx.accounts.program,
            args.memo,
        )
    }

    #[access_control(ctx.accounts.validate(&ctx, Vote::Cancel, args.voter_key))]
    pub fn cancel_proposal_v2(mut ctx: Context<Self>, args: VoteOnProposalV2Args) -> Result<()> {
        verify_vote_v2(&mut ctx, &args, Vote::Cancel)?;

        proposal_vote_inner(
            &mut ctx.accounts.consensus_account,
            &mut ctx.accounts.proposal,
            args.voter_key,
            Vote::Cancel,
            ctx.accounts.payer.as_ref(),
            ctx.accounts.system_program.as_ref(),
            &ctx.accounts.program,
            args.memo,
        )
    }
}

fn validate_vote(
    consensus_account: &InterfaceAccount<ConsensusAccount>,
    proposal: &Proposal,
    signer_key: Pubkey,
    vote: Vote,
    remaining_accounts: &[AccountInfo],
) -> Result<()> {
    consensus_account.is_active(remaining_accounts)?;

    require!(
        consensus_account.is_signer(signer_key).is_some(),
        SmartAccountError::NotASigner
    );
    require!(
        consensus_account.signer_has_permission(signer_key, Permission::Vote),
        SmartAccountError::Unauthorized
    );

    match vote {
        Vote::Approve | Vote::Reject => {
            require!(
                matches!(proposal.status, ProposalStatus::Active { .. }),
                SmartAccountError::InvalidProposalStatus
            );
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
        }
    }

    Ok(())
}

fn proposal_vote_inner<'info>(
    consensus_account: &mut InterfaceAccount<'info, ConsensusAccount>,
    proposal: &mut Account<'info, Proposal>,
    signer_key: Pubkey,
    vote: Vote,
    payer: Option<&Signer<'info>>,
    system_program: Option<&Program<'info, System>>,
    program: &Program<'info, SquadsSmartAccountProgram>,
    memo: Option<String>,
) -> Result<()> {
    match vote {
        Vote::Approve => {
            require!(
                proposal.approved.binary_search(&signer_key).is_err(),
                SmartAccountError::AlreadyApproved
            );
        }
        Vote::Reject => {
            require!(
                proposal.rejected.binary_search(&signer_key).is_err(),
                SmartAccountError::AlreadyRejected
            );
        }
        Vote::Cancel => {
            require!(
                proposal.cancelled.binary_search(&signer_key).is_err(),
                SmartAccountError::AlreadyCancelled
            );
        }
    }

    let threshold = usize::from(consensus_account.threshold());
    let cutoff = consensus_account.cutoff();

    match vote {
        Vote::Approve => {
            proposal.approve(signer_key, threshold)?;
        }
        Vote::Reject => {
            proposal.reject(signer_key, cutoff)?;
        }
        Vote::Cancel => {
            proposal
                .cancelled
                .retain(|k| consensus_account.is_signer(*k).is_some());

            proposal.cancel(signer_key, threshold)?;

            let payer = payer.ok_or(SmartAccountError::MissingAccount)?;
            let system_program = system_program.ok_or(SmartAccountError::MissingAccount)?;

            Proposal::realloc_if_needed(
                proposal.to_account_info().clone(),
                consensus_account.signers_len(),
                Some(payer.to_account_info().clone()),
                Some(system_program.to_account_info().clone()),
            )?;
        }
    }

    let event_type = match vote {
        Vote::Approve => ProposalEventType::Approve,
        Vote::Reject => ProposalEventType::Reject,
        Vote::Cancel => ProposalEventType::Cancel,
    };

    let vote_event = ProposalEvent {
        event_type,
        consensus_account: consensus_account.key(),
        consensus_account_type: consensus_account.account_type(),
        proposal_pubkey: proposal.key(),
        transaction_index: proposal.transaction_index,
        signer: Some(signer_key),
        memo,
        proposal: Some(Proposal::try_from_slice(&proposal.try_to_vec()?)?),
    };

    let log_authority_info = LogAuthorityInfo {
        authority: consensus_account.to_account_info(),
        authority_seeds: consensus_account.get_signer_seeds(),
        bump: consensus_account.bump(),
        program: program.to_account_info(),
    };
    SmartAccountEvent::ProposalEvent(vote_event).log(&log_authority_info)?;

    Ok(())
}

fn verify_vote_v2(
    ctx: &mut Context<VoteOnProposalV2<'_>>,
    args: &VoteOnProposalV2Args,
    vote: Vote,
) -> Result<()> {
    let expected_message = create_vote_message(
        &ctx.accounts.proposal.key(),
        vote_to_u8(vote),
        ctx.accounts.consensus_account.transaction_index(),
    );

    verify_v2_context(
        &mut ctx.accounts.consensus_account,
        args.voter_key,
        &ctx.remaining_accounts,
        &expected_message,
        args.client_data_params.as_ref(),
    )?;

    Ok(())
}

fn vote_to_u8(vote: Vote) -> u8 {
    match vote {
        Vote::Approve => 0,
        Vote::Reject => 1,
        Vote::Cancel => 2,
    }
}

pub(crate) enum Vote {
    Approve,
    Reject,
    Cancel,
}
