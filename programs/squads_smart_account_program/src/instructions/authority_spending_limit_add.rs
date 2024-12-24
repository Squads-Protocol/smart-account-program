use anchor_lang::prelude::*;

use crate::errors::*;
use crate::state::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddSpendingLimitArgs {
    /// Key that is used to seed the SpendingLimit PDA.
    pub seed: Pubkey,
    /// The index of the vault that the spending limit is for.
    pub account_index: u8,
    /// The token mint the spending limit is for.
    pub mint: Pubkey,
    /// The amount of tokens that can be spent in a period.
    /// This amount is in decimals of the mint,
    /// so 1 SOL would be `1_000_000_000` and 1 USDC would be `1_000_000`.
    pub amount: u64,
    /// The reset period of the spending limit.
    /// When it passes, the remaining amount is reset, unless it's `Period::OneTime`.
    pub period: Period,
    /// Signers of the Spending Limit that can use it.
    /// Don't have to be signers of the settings.
    pub signers: Vec<Pubkey>,
    /// The destination addresses the spending limit is allowed to sent funds to.
    /// If empty, funds can be sent to any address.
    pub destinations: Vec<Pubkey>,
    /// The expiration timestamp of the spending limit.
    /// Non expiring spending limits are set to `i64::MAX`.
    pub expiration: i64,
    /// Memo is used for indexing only.
    pub memo: Option<String>,
}

#[derive(Accounts)]
#[instruction(args: AddSpendingLimitArgs)]
pub struct AddSpendingLimitAsAuthority<'info> {
    #[account(
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    /// Settings `settings_authority` that must authorize the configuration change.
    pub settings_authority: Signer<'info>,

    #[account(
        init,
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_SPENDING_LIMIT,
            args.seed.as_ref(),
        ],
        bump,
        space = SpendingLimit::size(args.signers.len(), args.destinations.len()),
        payer = rent_payer
    )]
    pub spending_limit: Account<'info, SpendingLimit>,

    /// This is usually the same as `settings_authority`, but can be a different account if needed.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl AddSpendingLimitAsAuthority<'_> {
    fn validate(&self, expiration: i64) -> Result<()> {
        // settings_authority
        require_keys_eq!(
            self.settings_authority.key(),
            self.settings.settings_authority,
            SmartAccountError::Unauthorized
        );

        // `spending_limit` is partially checked via its seeds.

        // Expiration must be greater than the current timestamp.
        if expiration != i64::MAX {
            require!(
                expiration > Clock::get()?.unix_timestamp,
                SmartAccountError::SpendingLimitExpired
            );
        }

        Ok(())
    }

    /// Create a new spending limit for the controlled smart account.
    /// NOTE: This instruction must be called only by the `settings_authority` if one is set (Controlled Smart Account).
    ///       Uncontrolled Smart Accounts should use `create_settings_transaction` instead.
    #[access_control(ctx.accounts.validate(args.expiration))]
    pub fn add_spending_limit(ctx: Context<Self>, args: AddSpendingLimitArgs) -> Result<()> {
        let spending_limit = &mut ctx.accounts.spending_limit;

        // Make sure there are no duplicate keys in this direct invocation by sorting so the invariant will catch
        let mut sorted_signers = args.signers;
        sorted_signers.sort();

        spending_limit.settings = ctx.accounts.settings.key();
        spending_limit.seed = args.seed;
        spending_limit.account_index = args.account_index;
        spending_limit.mint = args.mint;
        spending_limit.amount = args.amount;
        spending_limit.period = args.period;
        spending_limit.remaining_amount = args.amount;
        spending_limit.last_reset = Clock::get()?.unix_timestamp;
        spending_limit.bump = ctx.bumps.spending_limit;
        spending_limit.signers = sorted_signers;
        spending_limit.destinations = args.destinations;
        spending_limit.expiration = args.expiration;

        spending_limit.invariant()?;

        Ok(())
    }
}
