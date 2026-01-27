use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::consensus_trait::ConsensusAccountType;
use crate::errors::*;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;
use crate::utils::{
    create_execute_settings_transaction_message, split_instructions_sysvar, verify_v2_context,
};
use crate::LogAuthorityInfo;
use crate::ProposalEvent;
use crate::ProposalEventType;
use crate::SmartAccountEvent;
use crate::TransactionContent;
use crate::TransactionEvent;
use crate::TransactionEventType;

#[derive(Accounts)]
pub struct ExecuteSettingsTransaction<'info> {
    /// The settings account of the smart account that owns the transaction.
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Box<Account<'info, Settings>>,

    /// The signer on the smart account that is executing the transaction.
    pub signer: Signer<'info>,

    /// The proposal account associated with the transaction.
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &transaction.index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        bump = proposal.bump,
    )]
    pub proposal: Account<'info, Proposal>,

    /// The transaction to execute.
    #[account(
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &transaction.index.to_le_bytes(),
        ],
        bump = transaction.bump,
    )]
    pub transaction: Account<'info, SettingsTransaction>,

    /// The account that will be charged/credited in case the settings transaction causes space reallocation,
    /// for example when adding a new signer, adding or removing a spending limit.
    /// This is usually the same as `signer`, but can be a different account if needed.
    #[account(mut)]
    pub rent_payer: Option<Signer<'info>>,

    /// We might need it in case reallocation is needed.
    pub system_program: Option<Program<'info, System>>,

    pub program: Program<'info, SquadsSmartAccountProgram>,

    // In case the transaction contains Add(Remove)SpendingLimit actions,
    // `remaining_accounts` must contain the SpendingLimit accounts to be initialized/closed.
    // remaining_accounts
}

impl<'info> ExecuteSettingsTransaction<'info> {
    fn validate(&self) -> Result<()> {
        validate_execute_settings_transaction(
            &self.settings,
            &self.proposal,
            &self.transaction,
            self.signer.key(),
        )
    }

    /// Execute the settings transaction.
    /// The transaction must be `Approved`.
    #[access_control(ctx.accounts.validate())]
    pub fn execute_settings_transaction(ctx: Context<'_, '_, 'info, 'info, Self>) -> Result<()> {
        execute_settings_transaction_inner(
            &mut ctx.accounts.settings,
            &mut ctx.accounts.proposal,
            &ctx.accounts.transaction,
            &ctx.accounts.program,
            ctx.accounts.signer.key(),
            &ctx.accounts.rent_payer,
            &ctx.accounts.system_program,
            &ctx.remaining_accounts,
            &ctx.program_id,
        )
    }
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ExecuteSettingsTransactionV2Args {
    /// The key (Native) or key_id (External) of the executor
    pub executor_key: Pubkey,
    /// Client data params for WebAuthn verification (required for WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
}

#[derive(Accounts)]
pub struct ExecuteSettingsTransactionV2<'info> {
    /// The settings account of the smart account that owns the transaction.
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Box<Account<'info, Settings>>,

    /// The proposal account associated with the transaction.
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &transaction.index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        bump = proposal.bump,
    )]
    pub proposal: Account<'info, Proposal>,

    /// The transaction to execute.
    #[account(
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &transaction.index.to_le_bytes(),
        ],
        bump = transaction.bump,
    )]
    pub transaction: Account<'info, SettingsTransaction>,

    /// The account that will be charged/credited in case the settings transaction causes space reallocation
    #[account(mut)]
    pub rent_payer: Option<Signer<'info>>,

    /// We might need it in case reallocation is needed.
    pub system_program: Option<Program<'info, System>>,

    pub program: Program<'info, SquadsSmartAccountProgram>,

    // remaining_accounts with optional instructions sysvar/native signers at front
}

impl<'info> ExecuteSettingsTransactionV2<'info> {
    fn validate(&self, args: &ExecuteSettingsTransactionV2Args) -> Result<()> {
        validate_execute_settings_transaction(
            &self.settings,
            &self.proposal,
            &self.transaction,
            args.executor_key,
        )
    }

    /// Execute the settings transaction with V2 signer support.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn execute_settings_transaction_v2(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: ExecuteSettingsTransactionV2Args,
    ) -> Result<()> {
        let expected_message = create_execute_settings_transaction_message(
            &ctx.accounts.transaction.key(),
            ctx.accounts.transaction.index,
        );

        verify_v2_context(
            &mut ctx.accounts.settings,
            args.executor_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        let (_, remaining_accounts) = split_instructions_sysvar(&ctx.remaining_accounts);

        execute_settings_transaction_inner(
            &mut ctx.accounts.settings,
            &mut ctx.accounts.proposal,
            &ctx.accounts.transaction,
            &ctx.accounts.program,
            args.executor_key,
            &ctx.accounts.rent_payer,
            &ctx.accounts.system_program,
            remaining_accounts,
            &ctx.program_id,
        )
    }
}

fn validate_execute_settings_transaction(
    settings: &Settings,
    proposal: &Proposal,
    transaction: &SettingsTransaction,
    signer_key: Pubkey,
) -> Result<()> {
    // signer
    require!(
        settings.is_signer(signer_key).is_some(),
        SmartAccountError::NotASigner
    );
    require!(
        settings.signer_has_permission(signer_key, Permission::Execute),
        SmartAccountError::Unauthorized
    );

    // proposal
    match proposal.status {
        ProposalStatus::Approved { timestamp } => {
            require!(
                Clock::get()?.unix_timestamp - timestamp >= i64::from(settings.time_lock),
                SmartAccountError::TimeLockNotReleased
            );
        }
        _ => return err!(SmartAccountError::InvalidProposalStatus),
    }
    // Stale settings transaction proposals CANNOT be executed even if approved.
    require!(
        proposal.transaction_index > settings.stale_transaction_index,
        SmartAccountError::StaleProposal
    );

    // Spending limit expiration must be greater than the current timestamp.
    let current_timestamp = Clock::get()?.unix_timestamp;

    for action in transaction.actions.iter() {
        if let SettingsAction::AddSpendingLimit { expiration, .. } = action {
            require!(
                *expiration > current_timestamp,
                SmartAccountError::SpendingLimitExpired
            );
        }
    }
    Ok(())
}

fn execute_settings_transaction_inner<'info>(
    settings: &mut Account<'info, Settings>,
    proposal: &mut Account<'info, Proposal>,
    transaction: &Account<'info, SettingsTransaction>,
    program: &Program<'info, SquadsSmartAccountProgram>,
    signer_key: Pubkey,
    rent_payer: &Option<Signer<'info>>,
    system_program: &Option<Program<'info, System>>,
    remaining_accounts: &'info [AccountInfo<'info>],
    program_id: &Pubkey,
) -> Result<()> {
    let settings_key = settings.key();

    let rent = Rent::get()?;

    // Log event authority info
    let log_authority_info = LogAuthorityInfo {
        authority: settings.to_account_info().clone(),
        authority_seeds: get_settings_signer_seeds(settings.seed),
        bump: settings.bump,
        program: program.to_account_info(),
    };

    // Execute the actions one by one.
    for action in transaction.actions.iter() {
        settings.modify_with_action(
            &settings_key,
            action,
            &rent,
            rent_payer,
            system_program,
            remaining_accounts,
            program_id,
            Some(&log_authority_info),
        )?;
    }

    // Make sure the smart account can fit the updated state: added signers or newly set rent_collector.
    Settings::realloc_if_needed_for_wrapper(
        settings.to_account_info(),
        &settings.signers,
        rent_payer.as_ref().map(ToAccountInfo::to_account_info),
        system_program.as_ref().map(ToAccountInfo::to_account_info),
    )?;

    // Make sure the settings state is valid after applying the actions.
    settings.invariant()?;

    // Mark the proposal as executed.
    proposal.status = ProposalStatus::Executed {
        timestamp: Clock::get()?.unix_timestamp,
    };

    // Transaction event
    let event = TransactionEvent {
        event_type: TransactionEventType::Execute,
        consensus_account: settings.key(),
        consensus_account_type: ConsensusAccountType::Settings,
        transaction_pubkey: transaction.key(),
        transaction_index: transaction.index,
        signer: Some(signer_key),
        transaction_content: Some(TransactionContent::SettingsTransaction {
            settings: settings.clone().into_inner(),
            transaction: transaction.clone().into_inner(),
            changes: transaction.actions.clone(),
        }),
        memo: None,
    };

    // Proposal event
    let proposal_event = ProposalEvent {
        event_type: ProposalEventType::Execute,
        consensus_account: settings.key(),
        consensus_account_type: ConsensusAccountType::Settings,
        proposal_pubkey: proposal.key(),
        transaction_index: transaction.index,
        signer: Some(signer_key),
        memo: None,
        proposal: Some(proposal.clone().into_inner()),
    };

    SmartAccountEvent::TransactionEvent(event).log(&log_authority_info)?;
    SmartAccountEvent::ProposalEvent(proposal_event).log(&log_authority_info)?;

    Ok(())
}
