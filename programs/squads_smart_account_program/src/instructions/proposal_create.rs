use anchor_lang::prelude::*;
use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::events::*;
use crate::interface::consensus::ConsensusAccount;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;
use crate::utils::{create_proposal_create_message, verify_v2_context};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateProposalArgs {
    /// Index of the smart account transaction this proposal is associated with.
    pub transaction_index: u64,
    /// Whether the proposal should be initialized with status `Draft`.
    pub draft: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateProposalV2Args {
    /// Index of the smart account transaction this proposal is associated with.
    pub transaction_index: u64,
    /// Whether the proposal should be initialized with status `Draft`.
    pub draft: bool,
    /// The key (Native) or key_id (External) of the proposer
    pub proposer_key: Pubkey,
    /// Client data params for WebAuthn verification (required for WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
    /// Optional memo
    pub memo: Option<String>,
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
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl CreateProposal<'_> {
    fn validate(&self, ctx: &Context<Self>, args: &CreateProposalArgs) -> Result<()> {
        validate_create_proposal(
            &self.consensus_account,
            args.transaction_index,
            self.creator.key(),
            &ctx.remaining_accounts,
        )
    }

    /// Create a new  proposal.
    #[access_control(ctx.accounts.validate(&ctx, &args))]
    pub fn create_proposal(ctx: Context<Self>, args: CreateProposalArgs) -> Result<()> {
        create_proposal_inner(
            &mut ctx.accounts.proposal,
            &ctx.accounts.consensus_account,
            &ctx.accounts.rent_payer,
            ctx.accounts.creator.key(),
            args.transaction_index,
            args.draft,
            ctx.bumps.proposal,
            &ctx.accounts.program,
        )
    }
}

#[derive(Accounts)]
#[instruction(args: CreateProposalV2Args)]
pub struct CreateProposalV2<'info> {
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

    /// The payer for the proposal account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl CreateProposalV2<'_> {
    fn validate(&self, ctx: &Context<Self>, args: &CreateProposalV2Args) -> Result<()> {
        validate_create_proposal(
            &self.consensus_account,
            args.transaction_index,
            args.proposer_key,
            &ctx.remaining_accounts,
        )
    }

    /// Create a new proposal with V2 signer support.
    #[access_control(ctx.accounts.validate(&ctx, &args))]
    pub fn create_proposal_v2(ctx: Context<Self>, args: CreateProposalV2Args) -> Result<()> {
        let expected_message = create_proposal_create_message(
            &ctx.accounts.consensus_account.key(),
            args.transaction_index,
            args.draft,
        );

        let proposer_key = verify_v2_context(
            &mut ctx.accounts.consensus_account,
            args.proposer_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        create_proposal_inner(
            &mut ctx.accounts.proposal,
            &ctx.accounts.consensus_account,
            &ctx.accounts.rent_payer,
            proposer_key,
            args.transaction_index,
            args.draft,
            ctx.bumps.proposal,
            &ctx.accounts.program,
        )
    }
}

fn create_proposal_inner<'info>(
    proposal: &mut Account<'info, Proposal>,
    consensus_account: &InterfaceAccount<'info, ConsensusAccount>,
    rent_payer: &Signer<'info>,
    signer_key: Pubkey,
    transaction_index: u64,
    draft: bool,
    proposal_bump: u8,
    program: &Program<'info, SquadsSmartAccountProgram>,
) -> Result<()> {
    proposal.settings = consensus_account.key();
    proposal.transaction_index = transaction_index;
    proposal.rent_collector = rent_payer.key();
    proposal.status = if draft {
        ProposalStatus::Draft {
            timestamp: Clock::get()?.unix_timestamp,
        }
    } else {
        ProposalStatus::Active {
            timestamp: Clock::get()?.unix_timestamp,
        }
    };
    proposal.bump = proposal_bump;
    proposal.approved = vec![];
    proposal.rejected = vec![];
    proposal.cancelled = vec![];

    let event = ProposalEvent {
        event_type: ProposalEventType::Create,
        consensus_account: consensus_account.key(),
        consensus_account_type: consensus_account.account_type(),
        proposal_pubkey: proposal.key(),
        transaction_index,
        signer: Some(signer_key),
        proposal: Some(proposal.clone().into_inner()),
        memo: None,
    };
    let log_authority_info = LogAuthorityInfo {
        authority: consensus_account.to_account_info(),
        authority_seeds: consensus_account.get_signer_seeds(),
        bump: consensus_account.bump(),
        program: program.to_account_info(),
    };
    SmartAccountEvent::ProposalEvent(event).log(&log_authority_info)?;

    Ok(())
}

fn validate_create_proposal(
    consensus_account: &InterfaceAccount<ConsensusAccount>,
    transaction_index: u64,
    signer_key: Pubkey,
    remaining_accounts: &[AccountInfo],
) -> Result<()> {
    // Check if the consensus account is active
    consensus_account.is_active(remaining_accounts)?;

    // We can only create a proposal for an existing transaction.
    require!(
        transaction_index <= consensus_account.transaction_index(),
        SmartAccountError::InvalidTransactionIndex
    );

    // We can't create a proposal for a stale transaction.
    require!(
        transaction_index > consensus_account.stale_transaction_index(),
        SmartAccountError::StaleProposal
    );

    // proposer has to be a signer on the smart account.
    require!(
        consensus_account.is_signer(signer_key).is_some(),
        SmartAccountError::NotASigner
    );

    // Must have at least one of the following permissions: Initiate or Vote.
    require!(
        consensus_account.signer_has_permission(signer_key, Permission::Initiate)
            || consensus_account.signer_has_permission(signer_key, Permission::Vote),
        SmartAccountError::Unauthorized
    );

    Ok(())
}
