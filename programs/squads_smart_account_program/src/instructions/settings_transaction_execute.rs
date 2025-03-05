use anchor_lang::prelude::*;

use crate::errors::*;
use crate::state::*;

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
    // In case the transaction contains Add(Remove)SpendingLimit actions,
    // `remaining_accounts` must contain the SpendingLimit accounts to be initialized/closed.
    // remaining_accounts
}

impl<'info> ExecuteSettingsTransaction<'info> {
    fn validate(&self) -> Result<()> {
        let Self {
            settings,
            proposal,
            signer,
            ..
        } = self;

        // signer
        require!(
            settings.is_signer(signer.key()).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            settings.signer_has_permission(signer.key(), Permission::Execute),
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

        // `transaction` is validated by its seeds.

        // Spending limit expiration must be greater than the current timestamp.
        let current_timestamp = Clock::get()?.unix_timestamp;

        for action in self.transaction.actions.iter() {
            if let SettingsAction::AddSpendingLimit { expiration, .. } = action {
                require!(
                    *expiration > current_timestamp,
                    SmartAccountError::SpendingLimitExpired
                );
            }
        }
        Ok(())
    }

    /// Execute the settings transaction.
    /// The transaction must be `Approved`.
    #[access_control(ctx.accounts.validate())]
    pub fn execute_settings_transaction(ctx: Context<'_, '_, 'info, 'info, Self>) -> Result<()> {
        let settings = &mut ctx.accounts.settings;
        let settings_key = settings.key();
        let transaction = &ctx.accounts.transaction;
        let proposal = &mut ctx.accounts.proposal;

        let rent = Rent::get()?;

        // Execute the actions one by one.
        for action in transaction.actions.iter() {
            settings.modify_with_action(
                &settings_key,
                action,
                &rent,
                &ctx.accounts.rent_payer,
                &ctx.accounts.system_program,
                &ctx.remaining_accounts,
                &ctx.program_id,
            )?;
        }

        // Make sure the smart account can fit the updated state: added signers or newly set rent_collector.
        Settings::realloc_if_needed(
            settings.to_account_info(),
            settings.signers.len(),
            ctx.accounts
                .rent_payer
                .as_ref()
                .map(ToAccountInfo::to_account_info),
            ctx.accounts
                .system_program
                .as_ref()
                .map(ToAccountInfo::to_account_info),
        )?;

        // Make sure the settings state is valid after applying the actions.
        settings.invariant()?;

        // Mark the proposal as executed.
        proposal.status = ProposalStatus::Executed {
            timestamp: Clock::get()?.unix_timestamp,
        };

        Ok(())
    }
}
