use anchor_lang::{prelude::*, Ids};
use anchor_spl::token_interface::{TokenAccount, TokenInterface};

use crate::errors::SmartAccountError;

pub struct TrackedTokenAccount<'info> {
    pub account: &'info AccountInfo<'info>,
    pub balance: u64,
    pub delegate: Option<Pubkey>,
    pub authority: Pubkey,
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
    let executing_account_info = accounts
        .iter()
        .find(|account| account.key() == executing_account)
        .unwrap();
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
            let token_account = InterfaceAccount::<TokenAccount>::try_from(account).unwrap();
            // Only track token accounts that are owned by the executing account
            if token_account.owner == executing_account {
                let balance = token_account.amount;
                let delegate = if let Some(delegate_key) = Option::from(token_account.delegate) {
                    Some(delegate_key)
                } else {
                    None
                };
                let authority = token_account.owner;
                tracked_token_accounts.push(TrackedTokenAccount {
                    account,
                    balance,
                    delegate,
                    authority,
                });
            }
        }
    }

    Balances {
        executing_account: tracked_executing_account,
        token_accounts: tracked_token_accounts,
    }
}

pub struct AllowedChange {
    pub mint: Pubkey,
    pub amount: u64,
}
impl<'info> Balances<'info> {
    pub fn evaluate_balance_changes(&self, allowed_change: AllowedChange) -> Result<()> {
        // Check the executing account
        let current_lamports = self.executing_account.account.lamports();
        // Ensure the executing account doesn't have less lamports
        require_gte!(
            current_lamports,
            self.executing_account.lamports,
            SmartAccountError::PlaceholderError
        );

        // Check all of the token accounts
        for tracked_token_account in &self.token_accounts {
            // Ensure that any tracked token account is not closed
            if tracked_token_account.account.data_is_empty() {
                return Err(SmartAccountError::PlaceholderError.into());
            }
            // Re-deserialize the token account
            let post_token_account =
                InterfaceAccount::<TokenAccount>::try_from(tracked_token_account.account).unwrap();
            match post_token_account.mint == allowed_change.mint {
                true => {
                    let minimum_balance = tracked_token_account
                        .balance
                        .saturating_sub(allowed_change.amount);
                    // Ensure the token account has no greater difference than the allowed change
                    require_gte!(
                        post_token_account.amount,
                        minimum_balance,
                        SmartAccountError::PlaceholderError
                    );
                }
                false => {
                    // Ensure the token account has the exact or greater balance
                    // than before
                    require_gte!(
                        post_token_account.amount,
                        tracked_token_account.balance,
                        SmartAccountError::PlaceholderError
                    );
                }
            }
            // Ensure the delegate and authority have not changed
            let post_delegate = Option::from(post_token_account.delegate);
            require!(
                post_delegate == tracked_token_account.delegate,
                SmartAccountError::PlaceholderError
            );
            require_eq!(
                post_token_account.owner,
                tracked_token_account.authority,
                SmartAccountError::PlaceholderError
            );
        }
        Ok(())
    }
}
