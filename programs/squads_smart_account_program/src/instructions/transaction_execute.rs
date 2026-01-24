use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::consensus_trait::ConsensusAccountType;
use crate::errors::*;
use crate::events::*;
use crate::interface::consensus::ConsensusAccount;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;
use crate::utils::{create_execute_transaction_message, verify_v2_context, *};

#[derive(Accounts)]
pub struct ExecuteTransaction<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    /// The proposal account associated with the transaction.
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
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
            consensus_account.key().as_ref(),
            SEED_TRANSACTION,
            &transaction.index.to_le_bytes(),
        ],
        bump
    )]
    pub transaction: Account<'info, Transaction>,

    pub signer: Signer<'info>,
    pub program: Program<'info, SquadsSmartAccountProgram>,
    // `remaining_accounts` must include the following accounts in the exact
    // order:
    // For transaction execution:
    // 1. AddressLookupTable accounts in the order they appear in `message.address_table_lookups`.
    // 2. Accounts in the order they appear in `message.account_keys`.
    // 3. Accounts in the order they appear in `message.address_table_lookups`.
    //
    // For policy execution:
    // 1. Settings account if the policy has a settings state expiration
    // 2. Any remaining accounts associated with the policy
}

impl<'info> ExecuteTransaction<'info> {
    fn validate(&self, ctx: &Context<ExecuteTransaction<'info>>) -> Result<()> {
        validate_execute_transaction(
            &self.consensus_account,
            &self.proposal,
            self.signer.key(),
            &ctx.remaining_accounts,
        )
    }

    /// Execute the smart account transaction.
    /// The transaction must be `Approved`.
    #[access_control(ctx.accounts.validate(&ctx))]
    pub fn execute_transaction(ctx: Context<'_, '_, 'info, 'info, Self>) -> Result<()> {
        execute_transaction_inner(
            &mut ctx.accounts.consensus_account,
            &mut ctx.accounts.proposal,
            &ctx.accounts.transaction,
            &ctx.accounts.program,
            ctx.accounts.signer.key(),
            &ctx.remaining_accounts,
            &ctx.program_id,
        )
    }
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ExecuteTransactionV2Args {
    /// The key (Native) or key_id (External) of the executor
    pub executor_key: Pubkey,
    /// Client data params for WebAuthn verification (required for WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
}

#[derive(Accounts)]
pub struct ExecuteTransactionV2<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    /// The proposal account associated with the transaction.
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
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
            consensus_account.key().as_ref(),
            SEED_TRANSACTION,
            &transaction.index.to_le_bytes(),
        ],
        bump
    )]
    pub transaction: Account<'info, Transaction>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
    // remaining_accounts same as ExecuteTransaction, with optional instructions sysvar/native signers at front
}

impl<'info> ExecuteTransactionV2<'info> {
    fn validate(&self, ctx: &Context<'_, '_, 'info, 'info, Self>, args: &ExecuteTransactionV2Args) -> Result<()> {
        validate_execute_transaction(
            &self.consensus_account,
            &self.proposal,
            args.executor_key,
            &ctx.remaining_accounts,
        )
    }

    /// Execute a smart account transaction with V2 signer support.
    #[access_control(ctx.accounts.validate(&ctx, &args))]
    pub fn execute_transaction_v2(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: ExecuteTransactionV2Args,
    ) -> Result<()> {
        let expected_message = create_execute_transaction_message(
            &ctx.accounts.transaction.key(),
            ctx.accounts.transaction.index,
        );

        verify_v2_context(
            &mut ctx.accounts.consensus_account,
            args.executor_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        execute_transaction_inner(
            &mut ctx.accounts.consensus_account,
            &mut ctx.accounts.proposal,
            &ctx.accounts.transaction,
            &ctx.accounts.program,
            args.executor_key,
            &ctx.remaining_accounts,
            &ctx.program_id,
        )
    }
}

fn validate_execute_transaction(
    consensus_account: &InterfaceAccount<ConsensusAccount>,
    proposal: &Proposal,
    signer_key: Pubkey,
    remaining_accounts: &[AccountInfo],
) -> Result<()> {
    // Check if the consensus account is active
    consensus_account.is_active(remaining_accounts)?;

    // signer
    require!(
        consensus_account.is_signer(signer_key).is_some(),
        SmartAccountError::NotASigner
    );
    require!(
        consensus_account.signer_has_permission(signer_key, Permission::Execute),
        SmartAccountError::Unauthorized
    );

    // proposal
    match proposal.status {
        ProposalStatus::Approved { timestamp } => {
            require!(
                Clock::get()?.unix_timestamp - timestamp
                    >= i64::from(consensus_account.time_lock()),
                SmartAccountError::TimeLockNotReleased
            );
        }
        _ => return err!(SmartAccountError::InvalidProposalStatus),
    }
    // Stale transaction proposals CAN be executed if they were approved
    // before becoming stale, hence no check for staleness here.

    Ok(())
}

fn execute_transaction_inner<'info>(
    consensus_account: &mut InterfaceAccount<'info, ConsensusAccount>,
    proposal: &mut Account<'info, Proposal>,
    transaction: &Account<'info, Transaction>,
    program: &Program<'info, SquadsSmartAccountProgram>,
    signer_key: Pubkey,
    remaining_accounts: &'info [AccountInfo<'info>],
    program_id: &Pubkey,
) -> Result<()> {
    let consensus_account_key = consensus_account.key();
    let transaction_key = transaction.key();
    let transaction_payload = &transaction.payload;

    // Log authority info
    let log_authority_info = LogAuthorityInfo {
        authority: consensus_account.to_account_info(),
        authority_seeds: consensus_account.get_signer_seeds(),
        bump: consensus_account.bump(),
        program: program.to_account_info(),
    };

    match consensus_account.account_type() {
        ConsensusAccountType::Settings => {
            let transaction_payload = transaction_payload.transaction_payload()?;
            let smart_account_seeds = &[
                SEED_PREFIX,
                consensus_account_key.as_ref(),
                SEED_SMART_ACCOUNT,
                &transaction_payload.account_index.to_le_bytes(),
            ];

            let (smart_account_key, smart_account_bump) =
                Pubkey::find_program_address(smart_account_seeds, program_id);

            let smart_account_signer_seeds = &[
                smart_account_seeds[0],
                smart_account_seeds[1],
                smart_account_seeds[2],
                smart_account_seeds[3],
                &[smart_account_bump],
            ];

            let num_lookups = transaction_payload.message.address_table_lookups.len();

            let message_account_infos = remaining_accounts
                .get(num_lookups..)
                .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;
            let address_lookup_table_account_infos = remaining_accounts
                .get(..num_lookups)
                .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;

            let (ephemeral_signer_keys, ephemeral_signer_seeds) = derive_ephemeral_signers(
                transaction_key,
                &transaction_payload.ephemeral_signer_bumps,
            );

            let executable_message = ExecutableTransactionMessage::new_validated(
                transaction_payload.message.clone(),
                message_account_infos,
                address_lookup_table_account_infos,
                &smart_account_key,
                &ephemeral_signer_keys,
            )?;

            let protected_accounts = &[proposal.key()];

            // Execute the transaction message instructions one-by-one.
            executable_message.execute_message(
                smart_account_signer_seeds,
                &ephemeral_signer_seeds,
                protected_accounts,
            )?;
        }
        ConsensusAccountType::Policy => {
            let policy_payload = transaction_payload.policy_payload()?;
            // Extract the policy from the consensus account and execute using dispatch
            let policy = consensus_account.policy()?;
            // Determine account offset based on policy expiration type
            let account_offset = policy
                .expiration
                .as_ref()
                .map(|exp| match exp {
                    // The settings is the first extra remaining account
                    PolicyExpiration::SettingsState(_) => 1,
                    _ => 0,
                })
                .unwrap_or(0);

            let remaining_accounts = &remaining_accounts[account_offset..];

            policy.execute(
                Some(transaction),
                Some(proposal),
                &policy_payload.payload,
                remaining_accounts,
            )?;

            // Policy may updated during execution, log the event
            let policy_event = PolicyEvent {
                event_type: PolicyEventType::UpdateDuringExecution,
                settings_pubkey: policy.settings,
                policy_pubkey: consensus_account_key,
                policy: Some(policy.clone()),
            };
            SmartAccountEvent::PolicyEvent(policy_event).log(&log_authority_info)?;
        }
    }

    // Mark the proposal as executed.
    proposal.status = ProposalStatus::Executed {
        timestamp: Clock::get()?.unix_timestamp,
    };

    // Check the account invariants
    consensus_account.invariant()?;

    // Log the execution event
    let execute_event = TransactionEvent {
        consensus_account: consensus_account.key(),
        consensus_account_type: consensus_account.account_type(),
        event_type: TransactionEventType::Execute,
        transaction_pubkey: transaction.key(),
        transaction_index: transaction.index,
        signer: Some(signer_key),
        memo: None,
        transaction_content: Some(TransactionContent::Transaction(transaction.clone().into_inner())),
    };

    // Log the proposal vote event with execution state
    let proposal_event = ProposalEvent {
        event_type: ProposalEventType::Execute,
        consensus_account: consensus_account.key(),
        consensus_account_type: consensus_account.account_type(),
        proposal_pubkey: proposal.key(),
        transaction_index: transaction.index,
        signer: Some(signer_key),
        memo: None,
        proposal: Some(proposal.clone().into_inner()),
    };
    SmartAccountEvent::TransactionEvent(execute_event).log(&log_authority_info)?;
    SmartAccountEvent::ProposalEvent(proposal_event).log(&log_authority_info)?;

    Ok(())
}
