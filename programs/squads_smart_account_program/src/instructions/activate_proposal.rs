use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::*;
use crate::utils::{create_proposal_activate_message, verify_v2_context};

// TODO: rework the signer to be: Signer 

#[derive(Accounts)]
pub struct ActivateProposal<'info> {
    #[account(
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
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

// TODO: is_valid_signer that accepts both V1 and V2 validations.

// if account_info.is_signer() = consensus.is_signer() if not means key_id
// then find key_id: enum -> we know if we need to use instruction introspection 
// or if we need to use the additional data.
//
// Then check if there is a session key active. If yes remaining account.signer() after the sysvar
//
// Then create the message since the message don't always need the additional data

// Check fucntion args: Takes in the key_id (signer or not), consensus, additional_args, remaining_accounts, 
// message (this message is the current one that we have without additional args. We add the additional args 
// in the function if needed or keep as it is)

impl ActivateProposal<'_> {
    fn validate(&self) -> Result<()> {
        validate_activate_proposal(&*self.settings, &self.proposal, self.signer.key())
    }

    /// Update status of a multisig proposal from `Draft` to `Active`.
    #[access_control(ctx.accounts.validate())]
    pub fn activate_proposal(ctx: Context<Self>) -> Result<()> {
        activate_proposal_inner(&mut ctx.accounts.proposal)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ActivateProposalV2Args {
    /// The key (Native) or key_id (External) of the activating signer
    pub activator_key: Pubkey, // TODO: NO NEED THIS BECAUSE ACTIVATOR KEY BECOMES SIGNER
    /// Client data params for WebAuthn verification (required for WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
}

// TODO: client_data_params -> extra_verification data: SmallVec(u16)[u8] 

#[derive(Accounts)]
pub struct ActivateProposalV2<'info> {
    #[account(
        mut,
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
}

impl ActivateProposalV2<'_> {
    fn validate(&self, args: &ActivateProposalV2Args) -> Result<()> {
        
        validate_activate_proposal(&*self.consensus_account, &self.proposal, args.activator_key)
    }

    /// Update status of a multisig proposal from `Draft` to `Active` with V2 signer support.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn activate_proposal_v2(ctx: Context<Self>, args: ActivateProposalV2Args) -> Result<()> {
        let expected_message = create_proposal_activate_message(
            &ctx.accounts.proposal.key(),
            ctx.accounts.proposal.transaction_index,
        );

        verify_v2_context(
            &mut ctx.accounts.consensus_account,
            args.activator_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        activate_proposal_inner(&mut ctx.accounts.proposal)
    }
}

// TODO: use a message with the discriminator

fn validate_activate_proposal<C: Consensus>(
    consensus: &C,
    proposal: &Proposal,
    signer_key: Pubkey
) -> Result<()> {
    // Signer is part of the settings
    require!(
        consensus.is_signer(signer_key).is_some(),
        SmartAccountError::NotASigner
    );
    require!(
        // We consider this action a part of the proposal initiation.
        consensus.signer_has_permission(signer_key, Permission::Initiate),
        SmartAccountError::Unauthorized
    );

    // Proposal must be in draft status and not stale
    require!(
        matches!(proposal.status, ProposalStatus::Draft { .. }),
        SmartAccountError::InvalidProposalStatus
    );
    require!(
        proposal.transaction_index > consensus.stale_transaction_index(),
        SmartAccountError::StaleProposal
    );

    Ok(())
}

fn activate_proposal_inner(proposal: &mut Proposal) -> Result<()> {
    proposal.status = ProposalStatus::Active {
        timestamp: Clock::get()?.unix_timestamp,
    };

    Ok(())
}
