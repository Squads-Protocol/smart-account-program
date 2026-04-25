use solana_program::pubkey::Pubkey;

use crate::errors::SmartAccountError;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
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
    pub amount: u64,

    /// The reset period of the spending limit.
    pub period: Period,

    /// The remaining amount of tokens that can be spent in the current period.
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
    pub const DISCRIMINATOR: [u8; 8] = [0x0a, 0xc9, 0x1b, 0xa0, 0xda, 0xc3, 0xde, 0x98];

    pub fn size(signers_length: usize, destinations_length: usize) -> usize {
        8  + // discriminator
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
        8 // expiration
    }

    pub fn invariant(&self) -> Result<(), SmartAccountError> {
        if self.amount == 0 {
            return Err(SmartAccountError::SpendingLimitInvalidAmount);
        }
        if self.signers.is_empty() {
            return Err(SmartAccountError::EmptySigners);
        }
        let has_duplicates = self.signers.windows(2).any(|win| win[0] == win[1]);
        if has_duplicates {
            return Err(SmartAccountError::DuplicateSigner);
        }
        Ok(())
    }
}

#[cfg(feature = "borsh")]
impl SpendingLimit {
    /// Deserialize account data previously written by the anchor program,
    /// verifying the 8-byte discriminator before Borsh-decoding the body.
    pub fn try_deserialize(data: &[u8]) -> Result<Self, borsh::maybestd::io::Error> {
        if data.len() < 8 {
            return Err(borsh::maybestd::io::Error::new(
                borsh::maybestd::io::ErrorKind::InvalidData,
                "account data shorter than discriminator",
            ));
        }
        if data[..8] != Self::DISCRIMINATOR {
            return Err(borsh::maybestd::io::Error::new(
                borsh::maybestd::io::ErrorKind::InvalidData,
                "discriminator mismatch",
            ));
        }
        let mut body = &data[8..];
        <Self as borsh::BorshDeserialize>::deserialize(&mut body)
    }
}

/// The reset period of the spending limit.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
