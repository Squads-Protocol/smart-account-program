use anchor_lang::prelude::*;

use crate::consensus_trait::ConsensusAccountType;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::*;
use crate::utils::*;

#[derive(Accounts)]
pub struct ExecuteBatchTransaction<'info> {
    /// Consensus account this batch belongs to.
    #[account(
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok(),
        // Batches currenlty don't support policies
        constraint = consensus_account.account_type() == ConsensusAccountType::Settings
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    /// Signer of the settings.
    pub signer: Signer<'info>,

    /// The proposal account associated with the batch.
    /// If `transaction` is the last in the batch, the `proposal` status will be set to `Executed`.
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION,
            &batch.index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        bump = proposal.bump,
    )]
    pub proposal: Account<'info, Proposal>,

    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION,
            &batch.index.to_le_bytes(),
        ],
        bump = batch.bump,
    )]
    pub batch: Account<'info, Batch>,

    /// Batch transaction to execute.
    #[account(
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION,
            &batch.index.to_le_bytes(),
            SEED_BATCH_TRANSACTION,
            &batch.executed_transaction_index.checked_add(1).unwrap().to_le_bytes(),
        ],
        bump = transaction.bump,
    )]
    pub transaction: Account<'info, BatchTransaction>,
    //
    // `remaining_accounts` must include the following accounts in the exact order:
    // 1. AddressLookupTable accounts in the order they appear in `message.address_table_lookups`.
    // 2. Accounts in the order they appear in `message.account_keys`.
    // 3. Accounts in the order they appear in `message.address_table_lookups`.
}

impl ExecuteBatchTransaction<'_> {
    fn validate(&self, ctx: &Context<Self>) -> Result<()> {
        let Self {
            consensus_account,
            signer,
            proposal,
            ..
        } = self;

        // Check if the consensus account is active
        consensus_account.is_active(&ctx.remaining_accounts)?;

        // `signer`
        require!(
            consensus_account.is_signer(signer.key()).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            consensus_account.signer_has_permission(signer.key(), Permission::Execute),
            SmartAccountError::Unauthorized
        );

        // `proposal`
        match proposal.status {
            ProposalStatus::Approved { timestamp } => {
                require!(
                    Clock::get()?.unix_timestamp - timestamp >= i64::from(consensus_account.time_lock()),
                    SmartAccountError::TimeLockNotReleased
                );
            }
            _ => return err!(SmartAccountError::InvalidProposalStatus),
        };
        // Stale batch transaction proposals CAN be executed if they were approved
        // before becoming stale, hence no check for staleness here.

        // `batch` is validated by its seeds.

        // `transaction` is validated by its seeds.

        Ok(())
    }

    /// Execute a transaction from the batch.
    #[access_control(ctx.accounts.validate(&ctx))]
    pub fn execute_batch_transaction(ctx: Context<Self>) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let proposal = &mut ctx.accounts.proposal;
        let batch = &mut ctx.accounts.batch;

        // NOTE: After `take()` is called, the BatchTransaction is reduced to
        // its default empty value, which means it should no longer be referenced or
        // used after this point to avoid faulty behavior.
        // Instead only make use of the returned `transaction` value.
        let transaction = ctx.accounts.transaction.take();

        let consensus_account_key = consensus_account.key();
        let batch_key = batch.key();

        let smart_account_seeds = &[
            SEED_PREFIX,
            consensus_account_key.as_ref(),
            SEED_SMART_ACCOUNT,
            &batch.account_index.to_le_bytes(),
            &[batch.account_bump],
        ];

        let transaction_message = transaction.message;
        let num_lookups = transaction_message.address_table_lookups.len();

        let message_account_infos = ctx
            .remaining_accounts
            .get(num_lookups..)
            .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;
        let address_lookup_table_account_infos = ctx
            .remaining_accounts
            .get(..num_lookups)
            .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;

        let smart_account_pubkey = Pubkey::create_program_address(smart_account_seeds, ctx.program_id).unwrap();

        let (ephemeral_signer_keys, ephemeral_signer_seeds) =
            derive_ephemeral_signers(batch_key, &transaction.ephemeral_signer_bumps);

        let executable_message = ExecutableTransactionMessage::new_validated(
            transaction_message,
            message_account_infos,
            address_lookup_table_account_infos,
            &smart_account_pubkey,
            &ephemeral_signer_keys,
        )?;

        let protected_accounts = &[proposal.key(), batch_key];

        // Execute the transaction message instructions one-by-one.
        // NOTE: `execute_message()` calls `self.to_instructions_and_accounts()`
        // which in turn calls `take()` on
        // `self.message.instructions`, therefore after this point no more
        // references or usages of `self.message` should be made to avoid
        // faulty behavior.
        executable_message.execute_message(
            smart_account_seeds,
            &ephemeral_signer_seeds,
            protected_accounts,
        )?;

        // Increment the executed transaction index.
        batch.executed_transaction_index = batch
            .executed_transaction_index
            .checked_add(1)
            .expect("overflow");

        // If this is the last transaction in the batch, set the proposal status to `Executed`.
        if batch.executed_transaction_index == batch.size {
            proposal.status = ProposalStatus::Executed {
                timestamp: Clock::get()?.unix_timestamp,
            };
        }

        batch.invariant()?;

        Ok(())
    }
}
