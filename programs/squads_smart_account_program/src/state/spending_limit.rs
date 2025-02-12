use anchor_lang::prelude::*;

use crate::errors::*;

#[account]
pub struct SpendingLimit {
    /// The settings this belongs to.
    pub settings: Pubkey,

    /// Key that is used to seed the SpendingLimit PDA.
    pub seed: Pubkey,

    /// The index of the smart account that the spending limit is for.
    pub account_index: u8,

    /// The token mint the spending limit is for.
    /// Pubkey::default() means SOL.
    /// use NATIVE_MINT for Wrapped SOL.
    pub mint: Pubkey,

    /// The amount of tokens that can be spent in a period.
    /// This amount is in decimals of the mint,
    /// so 1 SOL would be `1_000_000_000` and 1 USDC would be `1_000_000`.
    pub amount: u64,

    /// The reset period of the spending limit.
    /// When it passes, the remaining amount is reset, unless it's `Period::OneTime`.
    pub period: Period,

    /// The remaining amount of tokens that can be spent in the current period.
    /// When reaches 0, the spending limit cannot be used anymore until the period reset.
    pub remaining_amount: u64,

    /// Unix timestamp marking the last time the spending limit was reset (or created).
    pub last_reset: i64,

    /// PDA bump.
    pub bump: u8,

    /// Signers that can use the spending limit.
    pub signers: Vec<Pubkey>,

    /// The destination addresses the spending limit is allowed to sent funds to.
    /// If empty, funds can be sent to any address.
    pub destinations: Vec<Pubkey>,

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
            self.amount,
            0,
            SmartAccountError::SpendingLimitInvalidAmount
        );

        require!(!self.signers.is_empty(), SmartAccountError::EmptySigners);

        // There must be no duplicate signers, we make sure signers are sorted when creating a SpendingLimit.
        let has_duplicates = self.signers.windows(2).any(|win| win[0] == win[1]);
        require!(!has_duplicates, SmartAccountError::DuplicateSigner);

        Ok(())
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
