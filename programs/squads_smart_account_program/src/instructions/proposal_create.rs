use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::events::*;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;
use crate::instructions::*;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::create_proposal_create_message;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateProposalArgs {
    /// Index of the smart account transaction this proposal is associated with.
    pub transaction_index: u64,
    /// Whether the proposal should be initialized with status `Draft`.
    pub draft: bool,
}

#[derive(Accounts)]
#[instruction(args: CreateProposalArgs)]
pub struct CreateProposal<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    #[account(
        init,
        payer = rent_payer,
        space = Proposal::size(consensus_account.signers_len()),
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION,
            &args.transaction_index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        bump
    )]
    pub proposal: Account<'info, Proposal>,

    pub creator: ResolvedSigner<'info>,

    /// The payer for the proposal account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl CreateProposal<'_> {
    fn validate(
        &mut self,
        args: &CreateProposalArgs,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        let Self {
            consensus_account, creator, ..
        } = self;

        // Strip instructions sysvar (if at [0]) before is_active so
        // SettingsState expiration sees the Settings account at [0].
        let accounts_for_active = if remaining_accounts
            .first()
            .map_or(false, |acc| acc.key == &anchor_lang::solana_program::sysvar::instructions::ID)
        { &remaining_accounts[1..] } else { remaining_accounts };

        // Check if the consensus account is active
        consensus_account.is_active(accounts_for_active)?;

        // args
        // We can only create a proposal for an existing transaction.
        require!(
            args.transaction_index <= consensus_account.transaction_index(),
            SmartAccountError::InvalidTransactionIndex
        );

        // We can't create a proposal for a stale transaction.
        require!(
            args.transaction_index > consensus_account.stale_transaction_index(),
            SmartAccountError::StaleProposal
        );

        // Build message for external signer verification
        let message = create_proposal_create_message(
            &consensus_account.key(),
            args.transaction_index,
            args.draft,
        );

        // Resolve and verify signer (native, session key, or external) and check Initiate OR Vote permission
        // We pass None for permission and check separately since we need OR logic
        creator.verify(
            &mut **consensus_account,
            remaining_accounts,
            message,
            extra_verification_data.as_ref(),
            None,
        )?;

        // Must have at least one of the following permissions: Initiate or Vote.
        // Use resolved key (parent signer key for session keys) for permission check.
        let resolved_key = creator.resolved_key()?;
        require!(
            consensus_account.signer_has_permission(resolved_key, Permission::Initiate)
                || consensus_account.signer_has_permission(resolved_key, Permission::Vote),
            SmartAccountError::Unauthorized
        );

        Ok(())
    }

    /// Create a new proposal.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, None))]
    pub fn create_proposal(ctx: Context<Self>, args: CreateProposalArgs) -> Result<()> {
        Self::create_proposal_inner(ctx, args)
    }

    /// Create a new proposal with V2 signer support.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, extra_verification_data))]
    pub fn create_proposal_v2(ctx: Context<Self>, args: CreateProposalArgs, extra_verification_data: Option<ExtraVerificationData>) -> Result<()> {
        Self::create_proposal_inner(ctx, args)
    }

    fn create_proposal_inner(ctx: Context<Self>, args: CreateProposalArgs) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let consensus_account = &ctx.accounts.consensus_account;
        let rent_payer = &mut ctx.accounts.rent_payer;
        let creator = &ctx.accounts.creator;

        // Use resolved key (parent signer key for session keys) for event logging
        let resolved_key = creator.resolved_key()?;

        proposal.settings = consensus_account.key();
        proposal.transaction_index = args.transaction_index;
        proposal.rent_collector = rent_payer.key();
        proposal.status = if args.draft {
            ProposalStatus::Draft {
                timestamp: Clock::get()?.unix_timestamp,
            }
        } else {
            ProposalStatus::Active {
                timestamp: Clock::get()?.unix_timestamp,
            }
        };
        proposal.bump = ctx.bumps.proposal;
        proposal.approved = vec![];
        proposal.rejected = vec![];
        proposal.cancelled = vec![];

        // Log the event
        let event = ProposalEvent {
            event_type: ProposalEventType::Create,
            consensus_account: consensus_account.key(),
            consensus_account_type: consensus_account.account_type(),
            proposal_pubkey: proposal.key(),
            transaction_index: args.transaction_index,
            signer: Some(resolved_key),
            proposal: Some(proposal.clone().into_inner()),
            memo: None,
        };
        let log_authority_info = LogAuthorityInfo {
            authority: consensus_account.to_account_info(),
            authority_seeds: consensus_account.get_signer_seeds(),
            bump: consensus_account.bump(),
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::ProposalEvent(event).log(&log_authority_info)?;

        Ok(())
    }
}
