use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::consensus_trait::ConsensusAccountType;
use crate::errors::*;
use crate::events::*;
use crate::interface::consensus::ConsensusAccount;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;
use crate::utils::*;

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
        let Self {
            consensus_account,
            proposal,
            signer,
            ..
        } = self;

        // Check if the consensus account is active
        consensus_account.is_active(&ctx.remaining_accounts)?;

        // signer
        require!(
            consensus_account.is_signer(signer.key()).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            consensus_account.signer_has_permission(signer.key(), Permission::Execute),
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

        // `transaction` is validated by its seeds.

        Ok(())
    }

    /// Execute the smart account transaction.
    /// The transaction must be `Approved`.
    #[access_control(ctx.accounts.validate(&ctx))]
    pub fn execute_transaction(ctx: Context<'_, '_, 'info, 'info, Self>) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let proposal = &mut ctx.accounts.proposal;

        let transaction = &ctx.accounts.transaction;

        let settings_key = consensus_account.key();
        let transaction_key = transaction.key();
        let transaction_payload = &transaction.payload;

        match consensus_account.account_type() {
            ConsensusAccountType::Settings => {
                let transaction_payload = transaction_payload.transaction_payload()?;
                let smart_account_seeds = &[
                    SEED_PREFIX,
                    settings_key.as_ref(),
                    SEED_SMART_ACCOUNT,
                    &transaction_payload.account_index.to_le_bytes(),
                ];

                let (smart_account_key, smart_account_bump) =
                    Pubkey::find_program_address(smart_account_seeds, &ctx.program_id);

                let smart_account_signer_seeds = &[
                    smart_account_seeds[0],
                    smart_account_seeds[1],
                    smart_account_seeds[2],
                    smart_account_seeds[3],
                    &[smart_account_bump],
                ];

                let num_lookups = transaction_payload.message.address_table_lookups.len();

                let message_account_infos = &ctx
                    .remaining_accounts
                    .get(num_lookups..)
                    .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;
                let address_lookup_table_account_infos = &ctx
                    .remaining_accounts
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
                // NOTE: `execute_message()` calls `self.to_instructions_and_accounts()`
                // which in turn calls `take()` on
                // `self.message.instructions`, therefore after this point no more
                // references or usages of `self.message` should be made to avoid
                // faulty behavior.
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

                let remaining_accounts = &ctx.remaining_accounts[account_offset..];

                policy.execute(
                    Some(&transaction),
                    Some(&proposal),
                    &policy_payload.payload,
                    remaining_accounts,
                )?;
                msg!("Policy State: {:?}", policy.policy_state);
            }
        }

        // Mark the proposal as executed.
        proposal.status = ProposalStatus::Executed {
            timestamp: Clock::get()?.unix_timestamp,
        };

        // Check the account invariants
        consensus_account.invariant()?;

        // Log the execution event
        let log_authority_info = LogAuthorityInfo {
            authority: consensus_account.to_account_info(),
            authority_seeds: consensus_account.get_signer_seeds(),
            bump: consensus_account.bump(),
            program: ctx.accounts.program.to_account_info(),
        };
        // Log the execution event
        let execute_event = TransactionEvent {
            consensus_account: consensus_account.key(),
            consensus_account_type: consensus_account.account_type(),
            event_type: TransactionEventType::Execute,
            transaction_pubkey: ctx.accounts.transaction.key(),
            transaction_index: transaction.index,
            signer: Some(ctx.accounts.signer.key()),
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
            signer: Some(ctx.accounts.signer.key()),
            memo: None,
            proposal: Some(proposal.clone().into_inner()),
        };
        SmartAccountEvent::TransactionEvent(execute_event).log(&log_authority_info)?;
        SmartAccountEvent::ProposalEvent(proposal_event).log(&log_authority_info)?;

        Ok(())
    }
}
