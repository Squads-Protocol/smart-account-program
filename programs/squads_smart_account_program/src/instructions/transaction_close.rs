//! Contains instructions for closing accounts related to settings transactions,
//! transaction buffers and batches.
//!
//! The differences between the 3 is minor but still exist. For example,
//! a settings transaction's accounts can always be closed if the proposal is stale,
//! while for transaction buffers and batches it's not allowed if the proposal is stale but Approved,
//! because they still can be executed in such a case.
//!
//! The other reason we have 3 different instructions is purely related to Anchor API which
//! allows adding the `close` attribute only to `Account<'info, XXX>` types, which forces us
//! into having 3 different `Accounts` structs.
use anchor_lang::prelude::*;
use anchor_lang::system_program;

use crate::consensus::ConsensusAccount;
use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::state::*;

#[derive(Accounts)]
pub struct CloseSettingsTransaction<'info> {
    #[account(
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    /// CHECK: `seeds` and `bump` verify that the account is the canonical Proposal,
    ///         the logic within `settings_transaction_close` does the rest of the checks.
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &transaction.index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        bump,
    )]
    pub proposal: AccountInfo<'info>,

    /// SettingsTransaction corresponding to the `proposal`.
    #[account(
        mut,
        has_one = settings @ SmartAccountError::TransactionForAnotherSmartAccount,
        close = transaction_rent_collector
    )]
    pub transaction: Account<'info, SettingsTransaction>,

    /// The rent payer for the proposal account.
    /// CHECK: validated later inside of `close_settings_transaction`.
    #[account(mut)]
    pub proposal_rent_collector: AccountInfo<'info>,

    /// The rent collector.
    /// CHECK: We only need to validate the address.
    #[account(
        mut,
        address = transaction.rent_collector @ SmartAccountError::InvalidRentCollector,
    )]
    pub transaction_rent_collector: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

impl CloseSettingsTransaction<'_> {
    /// Closes a `SettingsTransaction` and the corresponding `Proposal`.
    /// `transaction` can be closed if either:
    /// - the `proposal` is in a terminal state: `Executed`, `Rejected`, or `Cancelled`.
    /// - the `proposal` is stale.
    pub fn close_settings_transaction(ctx: Context<Self>) -> Result<()> {
        let settings = &ctx.accounts.settings;
        let transaction = &ctx.accounts.transaction;
        let proposal = &mut ctx.accounts.proposal;
        let proposal_rent_collector = &ctx.accounts.proposal_rent_collector;

        let is_stale = transaction.index <= settings.stale_transaction_index;

        let proposal_account = if proposal.data.borrow().is_empty() {
            None
        } else {
            Some(Proposal::try_deserialize(
                &mut &**proposal.data.borrow_mut(),
            )?)
        };

        #[allow(deprecated)]
        let can_close = if let Some(proposal_account) = &proposal_account {
            match proposal_account.status {
                // Draft proposals can only be closed if stale,
                // so they can't be activated anymore.
                ProposalStatus::Draft { .. } => is_stale,
                // Active proposals can only be closed if stale,
                // so they can't be voted on anymore.
                ProposalStatus::Active { .. } => is_stale,
                // Approved proposals for ConfigTransactions can be closed if stale,
                // because they cannot be executed anymore.
                ProposalStatus::Approved { .. } => is_stale,
                // Rejected proposals can be closed.
                ProposalStatus::Rejected { .. } => true,
                // Executed proposals can be closed.
                ProposalStatus::Executed { .. } => true,
                // Cancelled proposals can be closed.
                ProposalStatus::Cancelled { .. } => true,
                // Should never really be in this state.
                ProposalStatus::Executing => false,
            }
        } else {
            // If no Proposal account exists then the ConfigTransaction can only be closed if stale
            is_stale
        };

        require!(can_close, SmartAccountError::InvalidProposalStatus);

        // Close the `proposal` account if exists.
        Proposal::close_if_exists(
            proposal_account,
            proposal.to_account_info(),
            proposal_rent_collector.clone(),
        )?;

        // Anchor will close the `transaction` account for us.
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CloseTransaction<'info> {
    #[account(
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    /// CHECK: `seeds` and `bump` verify that the account is the canonical Proposal,
    ///         the logic within `transaction_close` does the rest of the checks.
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION,
            &transaction.index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        bump,
    )]
    pub proposal: AccountInfo<'info>,

    /// Transaction corresponding to the `proposal`.
    #[account(
        mut,
        has_one = consensus_account @ SmartAccountError::TransactionForAnotherSmartAccount,
        close = transaction_rent_collector
    )]
    pub transaction: Account<'info, Transaction>,

    /// The rent collector for the proposal account.
    /// CHECK: validated later inside of `close_transaction`.
    #[account(mut)]
    pub proposal_rent_collector: AccountInfo<'info>,

    /// The rent collector.
    /// CHECK: We only need to validate the address.
    #[account(
        mut,
        address = transaction.rent_collector @ SmartAccountError::InvalidRentCollector,
    )]
    pub transaction_rent_collector: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

impl CloseTransaction<'_> {
    /// Closes a `Transaction` and the corresponding `Proposal`.
    /// `transaction` can be closed if either:
    /// - the `proposal` is in a terminal state: `Executed`, `Rejected`, or `Cancelled`.
    /// - the `proposal` is stale and not `Approved`.
    pub fn close_transaction(ctx: Context<Self>) -> Result<()> {
        let consensus_account = &ctx.accounts.consensus_account;
        let transaction = &ctx.accounts.transaction;
        let proposal = &mut ctx.accounts.proposal;
        let proposal_rent_collector = &ctx.accounts.proposal_rent_collector;

        let is_stale = transaction.index <= consensus_account.stale_transaction_index();

        let proposal_account = if proposal.data.borrow().is_empty() {
            None
        } else {
            Some(Proposal::try_deserialize(
                &mut &**proposal.data.borrow_mut(),
            )?)
        };

        #[allow(deprecated)]
        let can_close = if let Some(proposal_account) = &proposal_account {
            match proposal_account.status {
                // Draft proposals can only be closed if stale,
                // so they can't be activated anymore.
                ProposalStatus::Draft { .. } => is_stale,
                // Active proposals can only be closed if stale,
                // so they can't be voted on anymore.
                ProposalStatus::Active { .. } => is_stale,
                // Approved proposals for VaultTransactions cannot be closed even if stale,
                // because they still can be executed.
                ProposalStatus::Approved { .. } => false,
                // Rejected proposals can be closed.
                ProposalStatus::Rejected { .. } => true,
                // Executed proposals can be closed.
                ProposalStatus::Executed { .. } => true,
                // Cancelled proposals can be closed.
                ProposalStatus::Cancelled { .. } => true,
                // Should never really be in this state.
                ProposalStatus::Executing => false,
            }
        } else {
            // If no Proposal account exists then the VaultTransaction can only be closed if stale
            is_stale
        };

        require!(can_close, SmartAccountError::InvalidProposalStatus);

        // Close the `proposal` account if exists.
        Proposal::close_if_exists(
            proposal_account,
            proposal.to_account_info(),
            proposal_rent_collector.clone(),
        )?;

        // Anchor will close the `transaction` account for us.
        Ok(())
    }
}

//region CloseBatchTransaction
#[derive(Accounts)]
pub struct CloseBatchTransaction<'info> {
    #[account(
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    #[account(
        has_one = settings @ SmartAccountError::ProposalForAnotherSmartAccount,
    )]
    pub proposal: Account<'info, Proposal>,

    /// `Batch` corresponding to the `proposal`.
    #[account(
        mut,
        has_one = settings @ SmartAccountError::TransactionForAnotherSmartAccount,
        constraint = batch.index == proposal.transaction_index @ SmartAccountError::TransactionNotMatchingProposal,
    )]
    pub batch: Account<'info, Batch>,

    /// `BatchTransaction` account to close.
    /// The transaction must be the current last one in the batch.
    #[account(
        mut,
        close = transaction_rent_collector,
    )]
    pub transaction: Account<'info, BatchTransaction>,

    /// The rent collector.
    /// CHECK: We only need to validate the address.
    #[account(
        mut,
        address = transaction.rent_collector @ SmartAccountError::InvalidRentCollector,
    )]
    pub transaction_rent_collector: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

impl CloseBatchTransaction<'_> {
    fn validate(&self) -> Result<()> {
        let Self {
            settings,
            proposal,
            batch,
            transaction,
            ..
        } = self;

        // Transaction must be the last one in the batch.
        // We do it here instead of the Anchor macro because we want to throw a more specific error,
        // and the macro doesn't allow us to override the default "seeds constraint is violated" one.
        // First, derive the address of the last transaction as if provided transaction is the last one.
        let last_transaction_address = Pubkey::create_program_address(
            &[
                SEED_PREFIX,
                settings.key().as_ref(),
                SEED_TRANSACTION,
                &batch.index.to_le_bytes(),
                SEED_BATCH_TRANSACTION,
                // Last transaction index.
                &batch.size.to_le_bytes(),
                // We can assume the transaction bump is correct here.
                &transaction.bump.to_le_bytes(),
            ],
            &crate::id(),
        )
        .map_err(|_| SmartAccountError::TransactionNotLastInBatch)?;

        // Then compare it to the provided transaction address.
        require_keys_eq!(
            transaction.key(),
            last_transaction_address,
            SmartAccountError::TransactionNotLastInBatch
        );

        let is_proposal_stale = proposal.transaction_index <= settings.stale_transaction_index;

        #[allow(deprecated)]
        let can_close = match proposal.status {
            // Transactions of Draft proposals can only be closed if stale,
            // so the proposal can't be activated anymore.
            ProposalStatus::Draft { .. } => is_proposal_stale,
            // Transactions of Active proposals can only be closed if stale,
            // so the proposal can't be voted on anymore.
            ProposalStatus::Active { .. } => is_proposal_stale,
            // Transactions of Approved proposals for `Batch`es cannot be closed even if stale,
            // because they still can be executed.
            ProposalStatus::Approved { .. } => false,
            // Transactions of Rejected proposals can be closed.
            ProposalStatus::Rejected { .. } => true,
            // Transactions of Executed proposals can be closed.
            ProposalStatus::Executed { .. } => true,
            // Transactions of Cancelled proposals can be closed.
            ProposalStatus::Cancelled { .. } => true,
            // Should never really be in this state.
            ProposalStatus::Executing => false,
        };

        require!(can_close, SmartAccountError::InvalidProposalStatus);

        Ok(())
    }

    /// Closes a `BatchTransaction` belonging to the `batch` and `proposal`.
    /// Closing a transaction reduces the `batch.size` by 1.
    /// `transaction` must be closed in the order from the last to the first,
    /// and the operation is only allowed if any of the following conditions is met:
    /// - the `proposal` is in a terminal state: `Executed`, `Rejected`, or `Cancelled`.
    /// - the `proposal` is stale and not `Approved`.
    #[access_control(ctx.accounts.validate())]
    pub fn close_batch_transaction(ctx: Context<Self>) -> Result<()> {
        let batch = &mut ctx.accounts.batch;

        batch.size = batch.size.checked_sub(1).expect("overflow");

        // Anchor macro will close the `transaction` account for us.

        Ok(())
    }
}
//endregion

//region CloseBatch
#[derive(Accounts)]
pub struct CloseBatch<'info> {
    #[account(
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    // pub proposal: Account<'info, Proposal>,
    /// CHECK: `seeds` and `bump` verify that the account is the canonical Proposal,
    ///         the logic within `close_batch` does the rest of the checks.
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &batch.index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        bump,
    )]
    pub proposal: AccountInfo<'info>,

    /// `Batch` corresponding to the `proposal`.
    #[account(
        mut,
        has_one = settings @ SmartAccountError::TransactionForAnotherSmartAccount,
        close = batch_rent_collector
    )]
    pub batch: Account<'info, Batch>,

    /// The rent collector for the proposal account.
    /// CHECK: validated later inside of `close_batch`.
    #[account(mut)]
    pub proposal_rent_collector: AccountInfo<'info>,

    /// The rent collector.
    /// CHECK: We only need to validate the address.
    #[account(
        mut,
        address = batch.rent_collector.key() @ SmartAccountError::InvalidRentCollector,
    )]
    pub batch_rent_collector: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

impl CloseBatch<'_> {
    /// Closes Batch and the corresponding Proposal accounts for proposals in terminal states:
    /// `Executed`, `Rejected`, or `Cancelled` or stale proposals that aren't `Approved`.
    ///
    /// This instruction is only allowed to be executed when all `BatchTransaction` accounts
    /// in the `batch` are already closed: `batch.size == 0`.
    pub fn close_batch(ctx: Context<Self>) -> Result<()> {
        let settings = &ctx.accounts.settings;
        let batch = &ctx.accounts.batch;
        let proposal = &mut ctx.accounts.proposal;
        let proposal_rent_collector = &ctx.accounts.proposal_rent_collector;

        let is_stale = batch.index <= settings.stale_transaction_index;

        let proposal_account = if proposal.data.borrow().is_empty() {
            None
        } else {
            Some(Proposal::try_deserialize(
                &mut &**proposal.data.borrow_mut(),
            )?)
        };

        #[allow(deprecated)]
        let can_close = if let Some(proposal_account) = &proposal_account {
            match proposal_account.status {
                // Draft proposals can only be closed if stale,
                // so they can't be activated anymore.
                ProposalStatus::Draft { .. } => is_stale,
                // Active proposals can only be closed if stale,
                // so they can't be voted on anymore.
                ProposalStatus::Active { .. } => is_stale,
                // Approved proposals for `Batch`s cannot be closed even if stale,
                // because they still can be executed.
                ProposalStatus::Approved { .. } => false,
                // Rejected proposals can be closed.
                ProposalStatus::Rejected { .. } => true,
                // Executed proposals can be closed.
                ProposalStatus::Executed { .. } => true,
                // Cancelled proposals can be closed.
                ProposalStatus::Cancelled { .. } => true,
                // Should never really be in this state.
                ProposalStatus::Executing => false,
            }
        } else {
            // If no Proposal account exists then the Batch can only be closed if stale
            is_stale
        };

        require!(can_close, SmartAccountError::InvalidProposalStatus);

        // Batch must be empty.
        require_eq!(batch.size, 0, SmartAccountError::BatchNotEmpty);

        // Close the `proposal` account if exists.
        Proposal::close_if_exists(
            proposal_account,
            proposal.to_account_info(),
            proposal_rent_collector.clone(),
        )?;

        // Anchor will close the `batch` account for us.
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CloseEmptyPolicyTransaction<'info> {
    /// CHECK: We only need to validate the address.
    #[account(
        constraint = empty_policy.data_is_empty() @ SmartAccountError::InvalidEmptyPolicy,
        constraint = empty_policy.owner == &system_program::ID @ SmartAccountError::InvalidEmptyPolicy,
    )]
    pub empty_policy: AccountInfo<'info>,

    /// CHECK: `seeds` and `bump` verify that the account is the canonical Proposal,
    ///         the logic within `transaction_close` does the rest of the checks.
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            empty_policy.key().as_ref(),
            SEED_TRANSACTION,
            &transaction.index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        bump,
    )]
    pub proposal: AccountInfo<'info>,

    /// Transaction corresponding to the `proposal`.
    #[account(
        mut,
        constraint = transaction.consensus_account == empty_policy.key() @ SmartAccountError::TransactionForAnotherPolicy,
        close = transaction_rent_collector
    )]
    pub transaction: Account<'info, Transaction>,

    /// The rent collector for the proposal account.
    /// CHECK: validated later inside of `close_empty_policy_transaction`.
    #[account(mut)]
    pub proposal_rent_collector: AccountInfo<'info>,

    /// The rent collector.
    /// CHECK: We only need to validate the address.
    #[account(
        mut,
        address = transaction.rent_collector @ SmartAccountError::InvalidRentCollector,
    )]
    pub transaction_rent_collector: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

impl CloseEmptyPolicyTransaction<'_> {
    /// Closes a `Transaction` and the corresponding `Proposal` for
    /// empty/deleted policies.
    ///
    /// Since a policy can never exist at the same address again after being
    /// closed, any transaction & proposal associated with it can be closed.
    pub fn close_empty_policy_transaction(ctx: Context<Self>) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let proposal_rent_collector = &ctx.accounts.proposal_rent_collector;

        let proposal_account = if proposal.data.borrow().is_empty() {
            None
        } else {
            Some(Proposal::try_deserialize(
                &mut &**proposal.data.borrow_mut(),
            )?)
        };

        // Close the `proposal` account if exists.
        Proposal::close_if_exists(
            proposal_account,
            proposal.to_account_info(),
            proposal_rent_collector.clone(),
        )?;

        // Anchor will close the `transaction` account for us.
        Ok(())
    }
}
