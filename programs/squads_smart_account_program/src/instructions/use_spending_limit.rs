use anchor_lang::prelude::*;
use anchor_spl::token_2022::TransferChecked;
use anchor_spl::token_interface;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::errors::*;
use crate::state::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UseSpendingLimitArgs {
    /// Amount of tokens to transfer.
    pub amount: u64,
    /// Decimals of the token mint. Used for double-checking against incorrect order of magnitude of `amount`.
    pub decimals: u8,
    /// Memo used for indexing.
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct UseSpendingLimit<'info> {
    /// The settings the `spending_limit` belongs to.
    #[account(
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Box<Account<'info, Settings>>,

    pub signer: Signer<'info>,

    /// The SpendingLimit account to use.
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_SPENDING_LIMIT,
            spending_limit.seed.as_ref(),
        ],
        bump = spending_limit.bump,
    )]
    pub spending_limit: Account<'info, SpendingLimit>,

    /// Smart account to transfer tokens from.
    /// CHECK: All the required checks are done by checking the seeds.
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_SMART_ACCOUNT,
            &spending_limit.account_index.to_le_bytes(),
        ],
        bump
    )]
    pub smart_account: AccountInfo<'info>,

    /// Destination account to transfer tokens to.
    /// CHECK: We do the checks in `SpendingLimitUse::validate`.
    #[account(mut)]
    pub destination: AccountInfo<'info>,

    /// In case `spending_limit.mint` is SOL.
    pub system_program: Option<Program<'info, System>>,

    /// The mint of the tokens to transfer in case `spending_limit.mint` is an SPL token.
    /// CHECK: We do the checks in `UseSpendingLimit::validate`.
    pub mint: Option<InterfaceAccount<'info, Mint>>,

    /// Smart account token account to transfer tokens from in case `spending_limit.mint` is an SPL token.
    #[account(
        mut,
        token::mint = mint,
        token::authority = smart_account,
    )]
    pub smart_account_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    /// Destination token account in case `spending_limit.mint` is an SPL token.
    #[account(
        mut,
        token::mint = mint,
        token::authority = destination,
    )]
    pub destination_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    /// In case `spending_limit.mint` is an SPL token.
    pub token_program: Option<Interface<'info, TokenInterface>>,
}

impl UseSpendingLimit<'_> {
    fn validate(&self) -> Result<()> {
        let Self {
            signer,
            spending_limit,
            mint,
            ..
        } = self;

        // signer
        require!(
            spending_limit.signers.contains(&signer.key()),
            SmartAccountError::Unauthorized
        );

        // spending_limit - needs no checking.

        // mint
        if spending_limit.mint == Pubkey::default() {
            // SpendingLimit is for SOL, there should be no mint account in this case.
            require!(mint.is_none(), SmartAccountError::InvalidMint);
        } else {
            // SpendingLimit is for an SPL token, `mint` must match `spending_limit.mint`.
            require!(
                spending_limit.mint == mint.as_ref().unwrap().key(),
                SmartAccountError::InvalidMint
            );
        }

        // smart_account - checked in the #[account] attribute.

        // smart_account_token_account - checked in the #[account] attribute.

        // destination
        if !spending_limit.destinations.is_empty() {
            require!(
                spending_limit
                    .destinations
                    .contains(&self.destination.key()),
                SmartAccountError::InvalidDestination
            );
        }

        // destination_token_account - checked in the #[account] attribute.

        // Spending limit must not be expired.
        if spending_limit.expiration != i64::MAX {
            require!(
                spending_limit.expiration > Clock::get()?.unix_timestamp,
                SmartAccountError::SpendingLimitExpired
            );
        }

        Ok(())
    }

    /// Use a spending limit to transfer tokens from a smart account to a destination account.
    #[access_control(ctx.accounts.validate())]
    pub fn use_spending_limit(ctx: Context<Self>, args: UseSpendingLimitArgs) -> Result<()> {
        let spending_limit = &mut ctx.accounts.spending_limit;
        let smart_account = &mut ctx.accounts.smart_account;
        let destination = &mut ctx.accounts.destination;

        let settings_key = ctx.accounts.settings.key();
        let smart_account_bump = ctx.bumps.smart_account;
        let now = Clock::get()?.unix_timestamp;

        // Reset `spending_limit.remaining_amount` if the `spending_limit.period` has passed.
        if let Some(reset_period) = spending_limit.period.to_seconds() {
            let passed_since_last_reset = now.checked_sub(spending_limit.last_reset).unwrap();

            if passed_since_last_reset > reset_period {
                spending_limit.remaining_amount = spending_limit.amount;

                let periods_passed = passed_since_last_reset.checked_div(reset_period).unwrap();

                // last_reset = last_reset + periods_passed * reset_period,
                spending_limit.last_reset = spending_limit
                    .last_reset
                    .checked_add(periods_passed.checked_mul(reset_period).unwrap())
                    .unwrap();
            }
        }

        // Update `spending_limit.remaining_amount`.
        // This will also check if `amount` doesn't exceed `spending_limit.remaining_amount`.
        spending_limit.remaining_amount = spending_limit
            .remaining_amount
            .checked_sub(args.amount)
            .ok_or(SmartAccountError::SpendingLimitExceeded)?;

        // Transfer tokens.
        if spending_limit.mint == Pubkey::default() {
            // Transfer using the system_program::transfer.
            let system_program = &ctx
                .accounts
                .system_program
                .as_ref()
                .ok_or(SmartAccountError::MissingAccount)?;

            // Sanity check for the decimals. Similar to the one in token_interface::transfer_checked.
            require!(args.decimals == 9, SmartAccountError::DecimalsMismatch);

            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: smart_account.clone(),
                        to: destination.clone(),
                    },
                    &[&[
                        SEED_PREFIX,
                        settings_key.as_ref(),
                        SEED_SMART_ACCOUNT,
                        &spending_limit.account_index.to_le_bytes(),
                        &[smart_account_bump],
                    ]],
                ),
                args.amount,
            )?
        } else {
            // Transfer using the token_program::transfer_checked.
            let mint = &ctx
                .accounts
                .mint
                .as_ref()
                .ok_or(SmartAccountError::MissingAccount)?;
            let smart_account_token_account = &ctx
                .accounts
                .smart_account_token_account
                .as_ref()
                .ok_or(SmartAccountError::MissingAccount)?;
            let destination_token_account = &ctx
                .accounts
                .destination_token_account
                .as_ref()
                .ok_or(SmartAccountError::MissingAccount)?;
            let token_program = &ctx
                .accounts
                .token_program
                .as_ref()
                .ok_or(SmartAccountError::MissingAccount)?;

            msg!(
                "token_program {} mint {} smart account {} destination {} amount {} decimals {}",
                &token_program.key,
                &mint.key(),
                &smart_account.key,
                &destination.key,
                &args.amount,
                &args.decimals
            );

            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    token_program.to_account_info(),
                    TransferChecked {
                        from: smart_account_token_account.to_account_info(),
                        mint: mint.to_account_info(),
                        to: destination_token_account.to_account_info(),
                        authority: smart_account.clone(),
                    },
                    &[&[
                        SEED_PREFIX,
                        settings_key.as_ref(),
                        SEED_SMART_ACCOUNT,
                        &spending_limit.account_index.to_le_bytes(),
                        &[smart_account_bump],
                    ]],
                ),
                args.amount,
                args.decimals,
            )?;
        }

        Ok(())
    }
}
