use anchor_lang::prelude::*;

use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::*;

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

    /// The signer on the smart account that is creating the proposal.
    pub creator: Signer<'info>,

    /// The payer for the proposal account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl CreateProposal<'_> {
    fn validate(&self, ctx: &Context<Self>, args: &CreateProposalArgs) -> Result<()> {
        let Self {
            consensus_account, creator, ..
        } = self;
        let creator_key = creator.key();

        // Check if the consensus account is active
        consensus_account.is_active(&ctx.remaining_accounts)?;

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

        // creator
        // Has to be a signer on the smart account.
        require!(
            consensus_account.is_signer(creator.key()).is_some(),
            SmartAccountError::NotASigner
        );

        // Must have at least one of the following permissions: Initiate or Vote.
        require!(
            consensus_account
                .signer_has_permission(creator_key, Permission::Initiate)
                || consensus_account
                    .signer_has_permission(creator_key, Permission::Vote),
            SmartAccountError::Unauthorized
        );

        Ok(())
    }

    /// Create a new  proposal.
    #[access_control(ctx.accounts.validate(&ctx, &args))]
    pub fn create_proposal(ctx: Context<Self>, args: CreateProposalArgs) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let consensus_account = &ctx.accounts.consensus_account;
        let rent_payer = &mut ctx.accounts.rent_payer;

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

        Ok(())
    }
}
