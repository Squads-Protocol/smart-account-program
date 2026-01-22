use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::events::*;
use crate::interface::consensus::ConsensusAccount;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;
use crate::utils::{
    create_vote_message, split_instructions_sysvar, verify_external_signatures,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum VoteV2 {
    Approve = 0,
    Reject = 1,
    Cancel = 2,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ProposalVoteV2Args {
    /// The vote type
    pub vote: VoteV2,
    /// The key (Native) or key_id (External) of the voting signer
    pub voter_key: Pubkey,
    /// Optional memo
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct ProposalVoteV2<'info> {
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

    // remaining_accounts:
    // - If external signatures are used, the FIRST remaining account MUST be instructions sysvar
    //   (sysvar::instructions::ID), followed by native signer accounts.
    // - Otherwise, remaining_accounts contains only native signer accounts.
}

impl<'info> ProposalVoteV2<'info> {
    fn validate(&self, ctx: &Context<ProposalVoteV2<'info>>, args: &ProposalVoteV2Args) -> Result<()> {
        let consensus_account = &self.consensus_account;
        let proposal = &self.proposal;

        // Check if the consensus account is active
        consensus_account.is_active(ctx.remaining_accounts)?;

        // Verify the voter is a signer of the consensus account
        require!(
            consensus_account.is_signer(args.voter_key).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            consensus_account.signer_has_permission(args.voter_key, Permission::Vote),
            SmartAccountError::Unauthorized
        );

        // Validate proposal status based on vote type
        match args.vote {
            VoteV2::Approve | VoteV2::Reject => {
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
            VoteV2::Cancel => {
                require!(
                    matches!(proposal.status, ProposalStatus::Approved { .. }),
                    SmartAccountError::InvalidProposalStatus
                );
                // CAN cancel a stale proposal
            }
        }

        Ok(())
    }

    /// Vote on a proposal with V2 signer support (native + external)
    #[access_control(ctx.accounts.validate(&ctx, &args))]
    pub fn proposal_vote_v2(ctx: Context<ProposalVoteV2<'info>>, args: ProposalVoteV2Args) -> Result<()> {
        let consensus_account = &ctx.accounts.consensus_account;
        let proposal = &mut ctx.accounts.proposal;

        // Verify the voter hasn't already voted based on vote type
        match args.vote {
            VoteV2::Approve => {
                require!(
                    proposal.approved.binary_search(&args.voter_key).is_err(),
                    SmartAccountError::AlreadyApproved
                );
            }
            VoteV2::Reject => {
                require!(
                    proposal.rejected.binary_search(&args.voter_key).is_err(),
                    SmartAccountError::AlreadyRejected
                );
            }
            VoteV2::Cancel => {
                require!(
                    proposal.cancelled.binary_search(&args.voter_key).is_err(),
                    SmartAccountError::AlreadyCancelled
                );
            }
        }

        // Verify the signature (native or external)
        let remaining = ctx.remaining_accounts;
        let (instructions_sysvar_opt, native_accounts) =
            split_instructions_sysvar(remaining);

        // Get signer - we already validated existence in validate()
        let signer = consensus_account
            .is_signer_v2(args.voter_key)
            .ok_or(SmartAccountError::NotASigner)?;

        let is_native = native_accounts
            .iter()
            .any(|acc| acc.key == &args.voter_key && acc.is_signer);

        if is_native {
            // Native signer verified via AccountInfo.is_signer
        } else {
            // External signer - verify via precompile introspection
            let instructions_sysvar = instructions_sysvar_opt
                .ok_or(SmartAccountError::MissingPrecompileInstruction)?;

            let expected_message = create_vote_message(
                &proposal.key(),
                args.vote as u8,
                consensus_account.transaction_index(),
            );

            // signer is already SmartAccountSignerV2
            verify_external_signatures(
                instructions_sysvar,
                &[signer.clone()],
                &expected_message,
                Some(&[args.voter_key]),
            )?;
        }

        // Process the vote
        let threshold = usize::from(consensus_account.threshold());
        let cutoff = consensus_account.cutoff();

        match args.vote {
            VoteV2::Approve => {
                proposal.approve(args.voter_key, threshold)?;
            }
            VoteV2::Reject => {
                proposal.reject(args.voter_key, cutoff)?;
            }
            VoteV2::Cancel => {
                // Clean up stale cancelled votes
                proposal
                    .cancelled
                    .retain(|k| consensus_account.is_signer(*k).is_some());

                proposal.cancel(args.voter_key, threshold)?;

                // Reallocate if needed
                if let (Some(payer), Some(system_program)) =
                    (&ctx.accounts.payer, &ctx.accounts.system_program)
                {
                    Proposal::realloc_if_needed(
                        proposal.to_account_info().clone(),
                        consensus_account.signers_len(),
                        Some(payer.to_account_info().clone()),
                        Some(system_program.to_account_info().clone()),
                    )?;
                }
            }
        }

        // Determine event type
        let event_type = match args.vote {
            VoteV2::Approve => ProposalEventType::Approve,
            VoteV2::Reject => ProposalEventType::Reject,
            VoteV2::Cancel => ProposalEventType::Cancel,
        };

        // Log the vote event
        let vote_event = ProposalEvent {
            event_type,
            consensus_account: consensus_account.key(),
            consensus_account_type: consensus_account.account_type(),
            proposal_pubkey: proposal.key(),
            transaction_index: proposal.transaction_index,
            signer: Some(args.voter_key),
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
}
