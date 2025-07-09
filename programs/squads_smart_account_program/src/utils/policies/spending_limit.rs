use anchor_lang::prelude::*;

use crate::errors::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SpendingLimit {
    /// The token mint the spending limit is for.
    /// Pubkey::default() means SOL.
    /// use NATIVE_MINT for Wrapped SOL.
    pub mint: Pubkey,

    /// Cadence limitations
    pub period: Option<Period>,
    /// The max number of uses per period.
    pub uses_per_period: u64,
    /// The remaining number of uses. Either in this period or absolute.
    pub remaining_uses: u64,

    /// Amount limitations
    /// The amount of tokens that can be spent in a period.
    /// This amount is in decimals of the mint,
    /// so 1 SOL would be `1_000_000_000` and 1 USDC would be `1_000_000`.
    pub amount_per_period: u64,
    /// The amount of tokens that can be spent per use.
    pub amount_per_use: u64,
    /// If true, the amount_per_use or amount_per_period must be exactly the amount of tokens that are being spent.
    pub enforce_exact_amount: bool,


    /// The remaining amount of tokens that can be spent in the current period.
    /// When reaches 0, the spending limit cannot be used anymore until the period reset.
    pub remaining_amount: u64,
    // The remaining amount of tokens that can be spent between now and the
    // expiration, allowing for overflows across periods. Can only be used in conjunction with a start and expiration date.
    pub remaining_total_amount: u64,

    /// Unix timestamp marking the last time the spending limit was reset (or created).
    pub last_reset: i64,

    /// PDA bump.
    pub bump: u8,

    /// The destination addresses the spending limit is allowed to sent funds to.
    /// If empty, funds can be sent to any address.
    pub destinations: Vec<Pubkey>,

    /// The start timestamp of the spending limit.
    pub start: i64,

    /// The expiration timestamp of the spending limit.
    pub expiration: i64,
}

impl SpendingLimit {
    pub fn size(signers_length: usize, destinations_length: usize) -> usize {
        8  + // anchor discriminator
        32 + // settings
        32 + // seed
        1  + // account_index
        32 + // mint
        8  + // amount
        1  + // period
        8  + // remaining_amount
        8  + // last_reset
        1  + // bump
        4  + // signers vector length
        signers_length * 32 + // signers
        4  + // destinations vector length
        destinations_length * 32 + // destinations
        8    // expiration
    }

    pub fn invariant(&self) -> Result<()> {
        // Amount must be a non-zero value.
        require_neq!(
            self.amount_per_period,
            0,
            SmartAccountError::SpendingLimitInvalidAmount
        );


        Ok(())
    }
}

pub struct SpendingLimitSetupParams {
    pub mint: Pubkey,
    pub period: Period,
    pub uses_per_period: u64,
    pub amount_per_period: u64,
    pub amount_per_use: u64,
}
impl Policy for SpendingLimit {
    type SetupParams = ();
    type UseParams = ();

    fn invariant(&self) -> Result<()> {
        self.invariant()
    }
}
/// The reset period of the spending limit.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Period {
    /// The spending limit can only be used once.
    OneTime,
    /// The spending limit is reset every day.
    Day,
    /// The spending limit is reset every week (7 days).
    Week,
    /// The spending limit is reset every month (30 days).
    Month,
}

impl Period {
    pub fn to_seconds(&self) -> Option<i64> {
        match self {
            Period::OneTime => None,
            Period::Day => Some(24 * 60 * 60),
            Period::Week => Some(7 * 24 * 60 * 60),
            Period::Month => Some(30 * 24 * 60 * 60),
        }
    }
}
