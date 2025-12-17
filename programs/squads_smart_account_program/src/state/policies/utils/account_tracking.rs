use anchor_lang::{prelude::*, Ids};
use anchor_spl::token_interface::{TokenAccount, TokenInterface};

use crate::{
    errors::SmartAccountError, state::policies::utils::spending_limit_v2::SpendingLimitV2,
};

pub struct TrackedTokenAccount<'info> {
    pub account: &'info AccountInfo<'info>,
    pub balance: u64,
    pub delegate: Option<(Pubkey, u64)>,
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub close_authority: Option<Pubkey>,
}

pub struct TrackedExecutingAccount<'info> {
    pub account: &'info AccountInfo<'info>,
    pub lamports: u64,
}

pub struct Balances<'info> {
    pub executing_account: TrackedExecutingAccount<'info>,
    pub token_accounts: Vec<TrackedTokenAccount<'info>>,
}

/// Pre-check the balances of the executing account and any token accounts that are owned by it
pub fn check_pre_balances<'info>(
    executing_account: Pubkey,
    accounts: &'info [AccountInfo<'info>],
) -> Balances<'info> {
    let mut tracked_token_accounts = Vec::with_capacity(accounts.len());

    // Get the executing account info
    let executing_account_info = accounts
        .iter()
        .find(|account| account.key() == executing_account)
        .unwrap();

    // Track the executing account
    let tracked_executing_account = TrackedExecutingAccount {
        account: executing_account_info,
        lamports: executing_account_info.lamports(),
    };

    // Iterate over all accounts and track any given token accounts that are
    // owned by the executing account
    let token_program_ids = TokenInterface::ids();
    for account in accounts {
        // Only track accounts owned by a token program and that are writable
        if token_program_ids.contains(&account.owner) && account.is_writable {
            // This may fail for accounts that are not token accounts, so skip if it does
            let Ok(token_account) = InterfaceAccount::<TokenAccount>::try_from(account) else {
                continue;
            };
            // Only track token accounts that are owned by the executing account
            if token_account.owner == executing_account {
                let balance = token_account.amount;
                let delegate = if let Some(delegate_key) = Option::from(token_account.delegate) {
                    Some((delegate_key, token_account.delegated_amount))
                } else {
                    None
                };
                let authority = token_account.owner;
                let mint = token_account.mint;
                let close_authority = Option::from(token_account.close_authority);

                // Add the token account to the tracked token accounts
                tracked_token_accounts.push(TrackedTokenAccount {
                    account,
                    balance,
                    delegate,
                    authority,
                    mint,
                    close_authority,
                });
            }
        }
    }

    Balances {
        executing_account: tracked_executing_account,
        token_accounts: tracked_token_accounts,
    }
}

impl<'info> Balances<'info> {
    /// Evaluate balance changes against the spending limits
    pub fn evaluate_balance_changes(
        &self,
        spending_limits: &mut Vec<SpendingLimitV2>,
    ) -> Result<()> {
        // Get the current timestamp
        let current_timestamp = Clock::get()?.unix_timestamp;

        // Check the executing accounts lamports
        let current_lamports = self.executing_account.account.lamports();

        // Check the SOL spending limit
        // Note: Assumes spending limits have been deduplicated, and no two spending limits can have the same mint
        if let Some(spending_limit) = spending_limits.iter_mut().find(|spending_limit| {
            spending_limit.mint() == Pubkey::default()
                && spending_limit.is_active(current_timestamp).is_ok()
        }) {
            // Ensure the executing account doesn't have less lamports than the allowed change
            let minimum_balance = self
                .executing_account
                .lamports
                .saturating_sub(spending_limit.remaining_in_period());
            require_gte!(
                current_lamports,
                minimum_balance,
                SmartAccountError::ProgramInteractionInsufficientLamportAllowance
            );
            // If the executing account has a lower balance than before, decrement the spending limit
            if current_lamports < self.executing_account.lamports {
                spending_limit.decrement(self.executing_account.lamports - current_lamports);
            }
        } else {
            // Ensure the executing account doesn't have less lamports than before
            require_gte!(
                current_lamports,
                self.executing_account.lamports,
                SmartAccountError::ProgramInteractionModifiedIllegalBalance
            );
        }

        // Check all of the token accounts
        let token_program_ids = TokenInterface::ids();
        for tracked_token_account in &self.token_accounts {
            // Ensure that any tracked token account is not closed
            if tracked_token_account.account.data_is_empty() {
                return Err(
                    SmartAccountError::ProgramInteractionIllegalTokenAccountModification.into(),
                );
            }
            // Ensure the account is still owned by a token program
            require!(
                token_program_ids.contains(&tracked_token_account.account.owner),
                SmartAccountError::ProgramInteractionIllegalTokenAccountModification
            );
            // Re-deserialize the token account
            let post_token_account =
                InterfaceAccount::<TokenAccount>::try_from(tracked_token_account.account).unwrap();
            // Ensure the mint has not changed
            require_eq!(
                post_token_account.mint,
                tracked_token_account.mint,
                SmartAccountError::ProgramInteractionIllegalTokenAccountModification
            );

            // Find the spending limit for the token account if it exists and is active
            if let Some(spending_limit) = spending_limits.iter_mut().find(|spending_limit| {
                spending_limit.mint() == post_token_account.mint
                    && spending_limit.is_active(current_timestamp).is_ok()
            }) {
                {
                    // Saturating subtraction since remaining_amount could be
                    // higher than the balance
                    let minimum_balance = tracked_token_account
                        .balance
                        .saturating_sub(spending_limit.remaining_in_period());

                    // Ensure the token account has no greater difference than the allowed change
                    require_gte!(
                        post_token_account.amount,
                        minimum_balance,
                        SmartAccountError::ProgramInteractionInsufficientTokenAllowance
                    );

                    // If the token account has a lower balance than before, decrement the spending limit
                    if post_token_account.amount < tracked_token_account.balance {
                        spending_limit
                            .decrement(tracked_token_account.balance - post_token_account.amount);
                    }
                }
            } else {
                // Ensure the token account has the exact or greater balance
                // than before
                require_gte!(
                    post_token_account.amount,
                    tracked_token_account.balance,
                    SmartAccountError::ProgramInteractionModifiedIllegalBalance
                );
            }

            // Ensure the delegate and authority have not changed. Delegated
            // amount must not decrease (to prevent unauthorized spending via delegate)
            let post_delegate: Option<(Pubkey, u64)> =
                if let Some(delegate_key) = Option::from(post_token_account.delegate) {
                    Some((delegate_key, post_token_account.delegated_amount))
                } else {
                    None
                };
            match (post_delegate, tracked_token_account.delegate) {
                (Some(post_delegate), Some(tracked_delegate)) => {
                    require_eq!(post_delegate.0, tracked_delegate.0);
                    require_gte!(post_delegate.1, tracked_delegate.1);
                }
                (None, None) => {}
                _ => {
                    return Err(SmartAccountError::ProgramInteractionIllegalTokenAccountModification.into());
                }
            };
            require_eq!(
                post_token_account.owner,
                tracked_token_account.authority,
                SmartAccountError::ProgramInteractionIllegalTokenAccountModification
            );
            // Ensure the close_authority has not changed
            let post_close_authority: Option<Pubkey> =
                Option::from(post_token_account.close_authority);
            require!(
                post_close_authority == tracked_token_account.close_authority,
                SmartAccountError::ProgramInteractionIllegalTokenAccountModification
            );
        }
        Ok(())
    }
}
