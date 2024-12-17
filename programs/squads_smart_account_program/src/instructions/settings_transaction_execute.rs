use anchor_lang::prelude::*;

use crate::errors::*;
use crate::id;
use crate::state::*;
use crate::utils::*;

#[derive(Accounts)]
pub struct ExecuteSettingsTransaction<'info> {
    /// The settings account that owns the transaction.
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.as_ref()],
        bump = settings.bump,
    )]
    pub settings: Box<Account<'info, Settings>>,

    /// One of the multisig members with `Execute` permission.
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

    /// The account that will be charged/credited in case the config transaction causes space reallocation,
    /// for example when adding a new member, adding or removing a spending limit.
    /// This is usually the same as `member`, but can be a different account if needed.
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

        // member
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
        // Stale config transaction proposals CANNOT be executed even if approved.
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

    /// Execute the multisig transaction.
    /// The transaction must be `Approved`.
    #[access_control(ctx.accounts.validate())]
    pub fn execute_settings_transaction(ctx: Context<'_, '_, 'info, 'info, Self>) -> Result<()> {
        let settings = &mut ctx.accounts.settings;
        let transaction = &ctx.accounts.transaction;
        let proposal = &mut ctx.accounts.proposal;

        let rent = Rent::get()?;

        // Execute the actions one by one.
        for action in transaction.actions.iter() {
            match action {
                SettingsAction::AddSigner { new_signer } => {
                    settings.add_signer(new_signer.to_owned());

                    settings.invalidate_prior_transactions();
                }

                SettingsAction::RemoveSigner { old_signer } => {
                    settings.remove_signer(old_signer.to_owned())?;

                    settings.invalidate_prior_transactions();
                }

                SettingsAction::ChangeThreshold { new_threshold } => {
                    settings.threshold = *new_threshold;

                    settings.invalidate_prior_transactions();
                }

                SettingsAction::SetTimeLock { new_time_lock } => {
                    settings.time_lock = *new_time_lock;

                    settings.invalidate_prior_transactions();
                }

                SettingsAction::AddSpendingLimit {
                    seed,
                    account_index,
                    signers,
                    mint,
                    amount,
                    period,
                    destinations,
                    expiration,
                } => {
                    let (spending_limit_key, spending_limit_bump) = Pubkey::find_program_address(
                        &[
                            SEED_PREFIX,
                            settings.key().as_ref(),
                            SEED_SPENDING_LIMIT,
                            seed.as_ref(),
                        ],
                        ctx.program_id,
                    );

                    // Find the SpendingLimit account in `remaining_accounts`.
                    let spending_limit_info = ctx
                        .remaining_accounts
                        .iter()
                        .find(|acc| acc.key == &spending_limit_key)
                        .ok_or(SmartAccountError::MissingAccount)?;

                    // `rent_payer` and `system_program` must also be present.
                    let rent_payer = &ctx
                        .accounts
                        .rent_payer
                        .as_ref()
                        .ok_or(SmartAccountError::MissingAccount)?;
                    let system_program = &ctx
                        .accounts
                        .system_program
                        .as_ref()
                        .ok_or(SmartAccountError::MissingAccount)?;

                    // Initialize the SpendingLimit account.
                    create_account(
                        rent_payer,
                        spending_limit_info,
                        system_program,
                        &id(),
                        &rent,
                        SpendingLimit::size(signers.len(), destinations.len()),
                        vec![
                            SEED_PREFIX.to_vec(),
                            settings.key().as_ref().to_vec(),
                            SEED_SPENDING_LIMIT.to_vec(),
                            seed.as_ref().to_vec(),
                            vec![spending_limit_bump],
                        ],
                    )?;

                    let mut signers = signers.to_vec();
                    // Make sure signers are sorted.
                    signers.sort();

                    // Serialize the SpendingLimit data into the account info.
                    let spending_limit = SpendingLimit {
                        settings: settings.key().to_owned(),
                        seed: seed.to_owned(),
                        account_index: *account_index,
                        signers,
                        amount: *amount,
                        mint: *mint,
                        period: *period,
                        remaining_amount: *amount,
                        last_reset: Clock::get()?.unix_timestamp,
                        bump: spending_limit_bump,
                        destinations: destinations.to_vec(),
                        expiration: *expiration,
                    };

                    spending_limit.invariant()?;

                    spending_limit
                        .try_serialize(&mut &mut spending_limit_info.data.borrow_mut()[..])?;
                }

                SettingsAction::RemoveSpendingLimit {
                    spending_limit: spending_limit_key,
                } => {
                    // Find the SpendingLimit account in `remaining_accounts`.
                    let spending_limit_info = ctx
                        .remaining_accounts
                        .iter()
                        .find(|acc| acc.key == spending_limit_key)
                        .ok_or(SmartAccountError::MissingAccount)?;

                    // `rent_payer` must also be present.
                    let rent_payer = &ctx
                        .accounts
                        .rent_payer
                        .as_ref()
                        .ok_or(SmartAccountError::MissingAccount)?;

                    let spending_limit = Account::<SpendingLimit>::try_from(spending_limit_info)?;

                    // SpendingLimit must belong to the `settings`.
                    require_keys_eq!(
                        spending_limit.settings,
                        settings.key(),
                        SmartAccountError::InvalidAccount
                    );

                    spending_limit.close(rent_payer.to_account_info())?;

                    // We don't need to invalidate prior transactions here because adding
                    // a spending limit doesn't affect the consensus parameters of the multisig.
                }

                SettingsAction::SetRentCollector { new_rent_collector } => {
                    settings.rent_collector = *new_rent_collector;

                    // We don't need to invalidate prior transactions here because changing
                    // `rent_collector` doesn't affect the consensus parameters of the multisig.
                }
            }
        }

        // Make sure the multisig account can fit the updated state: added members or newly set rent_collector.
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

        // Make sure the multisig state is valid after applying the actions.
        settings.invariant()?;

        // Mark the proposal as executed.
        proposal.status = ProposalStatus::Executed {
            timestamp: Clock::get()?.unix_timestamp,
        };

        Ok(())
    }
}
