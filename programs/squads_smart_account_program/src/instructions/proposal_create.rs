use anchor_lang::prelude::*;

use crate::errors::*;
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
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    #[account(
        init,
        payer = rent_payer,
        space = Proposal::size(settings.signers.len()),
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
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
    fn validate(&self, args: &CreateProposalArgs) -> Result<()> {
        let Self {
            settings, creator, ..
        } = self;
        let creator_key = creator.key();

        // args
        // We can only create a proposal for an existing transaction.
        require!(
            args.transaction_index <= settings.transaction_index,
            SmartAccountError::InvalidTransactionIndex
        );

        // We can't create a proposal for a stale transaction.
        require!(
            args.transaction_index > settings.stale_transaction_index,
            SmartAccountError::StaleProposal
        );

        // creator
        // Has to be a signer on the smart account.
        require!(
            self.settings.is_signer(self.creator.key()).is_some(),
            SmartAccountError::NotASigner
        );

        // Must have at least one of the following permissions: Initiate or Vote.
        require!(
            self.settings
                .signer_has_permission(creator_key, Permission::Initiate)
                || self
                    .settings
                    .signer_has_permission(creator_key, Permission::Vote),
            SmartAccountError::Unauthorized
        );

        Ok(())
    }

    /// Create a new  proposal.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn create_proposal(ctx: Context<Self>, args: CreateProposalArgs) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let settings = &ctx.accounts.settings;

        proposal.settings = settings.key();
        proposal.transaction_index = args.transaction_index;
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
