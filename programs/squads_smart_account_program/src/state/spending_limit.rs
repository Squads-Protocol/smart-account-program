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
        8 // expiration
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

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Priority 3: Spending Limit Validation Tests
    // ========================================================================

    #[test]
    fn test_period_to_seconds() {
        assert_eq!(Period::OneTime.to_seconds(), None);
        assert_eq!(Period::Day.to_seconds(), Some(86400));
        assert_eq!(Period::Week.to_seconds(), Some(604800));
        assert_eq!(Period::Month.to_seconds(), Some(2592000));
    }

    #[test]
    fn test_spending_limit_size_calculation() {
        let size_no_signers_no_destinations = SpendingLimit::size(0, 0);
        let size_with_signers = SpendingLimit::size(3, 0);
        let size_with_destinations = SpendingLimit::size(0, 2);
        let size_with_both = SpendingLimit::size(3, 2);

        // Each signer is 32 bytes, each destination is 32 bytes
        assert_eq!(size_with_signers - size_no_signers_no_destinations, 3 * 32);
        assert_eq!(size_with_destinations - size_no_signers_no_destinations, 2 * 32);
        assert_eq!(size_with_both - size_no_signers_no_destinations, 3 * 32 + 2 * 32);
    }

    #[test]
    fn test_spending_limit_invariant_valid() {
        let spending_limit = SpendingLimit {
            settings: Pubkey::new_unique(),
            seed: Pubkey::new_unique(),
            account_index: 0,
            mint: Pubkey::default(), // SOL
            amount: 1000,
            period: Period::Day,
            remaining_amount: 1000,
            last_reset: 1000000,
            bump: 0,
            signers: vec![Pubkey::new_unique(), Pubkey::new_unique()],
            destinations: vec![],
            expiration: 2000000,
        };

        assert!(spending_limit.invariant().is_ok());
    }

    #[test]
    fn test_spending_limit_invariant_zero_amount() {
        let spending_limit = SpendingLimit {
            settings: Pubkey::new_unique(),
            seed: Pubkey::new_unique(),
            account_index: 0,
            mint: Pubkey::default(),
            amount: 0, // Invalid: zero amount
            period: Period::Day,
            remaining_amount: 0,
            last_reset: 1000000,
            bump: 0,
            signers: vec![Pubkey::new_unique()],
            destinations: vec![],
            expiration: 2000000,
        };

        assert!(spending_limit.invariant().is_err());
    }

    #[test]
    fn test_spending_limit_invariant_empty_signers() {
        let spending_limit = SpendingLimit {
            settings: Pubkey::new_unique(),
            seed: Pubkey::new_unique(),
            account_index: 0,
            mint: Pubkey::default(),
            amount: 1000,
            period: Period::Day,
            remaining_amount: 1000,
            last_reset: 1000000,
            bump: 0,
            signers: vec![], // Invalid: empty signers
            destinations: vec![],
            expiration: 2000000,
        };

        assert!(spending_limit.invariant().is_err());
    }

    #[test]
    fn test_spending_limit_invariant_duplicate_signers() {
        let duplicate_signer = Pubkey::new_unique();
        let spending_limit = SpendingLimit {
            settings: Pubkey::new_unique(),
            seed: Pubkey::new_unique(),
            account_index: 0,
            mint: Pubkey::default(),
            amount: 1000,
            period: Period::Day,
            remaining_amount: 1000,
            last_reset: 1000000,
            bump: 0,
            signers: vec![duplicate_signer, duplicate_signer], // Invalid: duplicates
            destinations: vec![],
            expiration: 2000000,
        };

        assert!(spending_limit.invariant().is_err());
    }

    #[test]
    fn test_spending_limit_invariant_sorted_signers_no_duplicates() {
        let key1 = Pubkey::new_unique();
        let key2 = Pubkey::new_unique();
        let mut sorted_keys = vec![key1, key2];
        sorted_keys.sort();

        let spending_limit = SpendingLimit {
            settings: Pubkey::new_unique(),
            seed: Pubkey::new_unique(),
            account_index: 0,
            mint: Pubkey::default(),
            amount: 1000,
            period: Period::Day,
            remaining_amount: 1000,
            last_reset: 1000000,
            bump: 0,
            signers: sorted_keys,
            destinations: vec![],
            expiration: 2000000,
        };

        assert!(spending_limit.invariant().is_ok());
    }

    #[test]
    fn test_spending_limit_period_reset_boundary() {
        // Test that periods have correct second conversions
        let day_seconds = Period::Day.to_seconds().unwrap();
        let week_seconds = Period::Week.to_seconds().unwrap();
        let month_seconds = Period::Month.to_seconds().unwrap();

        assert_eq!(day_seconds, 86400);
        assert_eq!(week_seconds, 604800);
        assert_eq!(month_seconds, 2592000);

        // Verify relationships
        assert_eq!(week_seconds, day_seconds * 7);
        assert_eq!(month_seconds, day_seconds * 30);
    }

    #[test]
    fn test_spending_limit_remaining_amount_tracking() {
        let mut spending_limit = SpendingLimit {
            settings: Pubkey::new_unique(),
            seed: Pubkey::new_unique(),
            account_index: 0,
            mint: Pubkey::default(),
            amount: 1000,
            period: Period::Day,
            remaining_amount: 1000,
            last_reset: 1000000,
            bump: 0,
            signers: vec![Pubkey::new_unique()],
            destinations: vec![],
            expiration: 2000000,
        };

        // Simulate spending
        spending_limit.remaining_amount = 500;
        assert_eq!(spending_limit.remaining_amount, 500);

        // Simulate reset
        spending_limit.remaining_amount = spending_limit.amount;
        assert_eq!(spending_limit.remaining_amount, spending_limit.amount);
    }

    #[test]
    fn test_spending_limit_one_time_period() {
        let spending_limit = SpendingLimit {
            settings: Pubkey::new_unique(),
            seed: Pubkey::new_unique(),
            account_index: 0,
            mint: Pubkey::default(),
            amount: 1000,
            period: Period::OneTime,
            remaining_amount: 1000,
            last_reset: 1000000,
            bump: 0,
            signers: vec![Pubkey::new_unique()],
            destinations: vec![],
            expiration: 2000000,
        };

        // OneTime should have no reset period
        assert_eq!(spending_limit.period.to_seconds(), None);
        assert!(spending_limit.invariant().is_ok());
    }

    #[test]
    fn test_spending_limit_destination_validation() {
        let dest1 = Pubkey::new_unique();
        let dest2 = Pubkey::new_unique();

        let spending_limit = SpendingLimit {
            settings: Pubkey::new_unique(),
            seed: Pubkey::new_unique(),
            account_index: 0,
            mint: Pubkey::default(),
            amount: 1000,
            period: Period::Day,
            remaining_amount: 1000,
            last_reset: 1000000,
            bump: 0,
            signers: vec![Pubkey::new_unique()],
            destinations: vec![dest1, dest2],
            expiration: 2000000,
        };

        assert!(spending_limit.invariant().is_ok());
        assert_eq!(spending_limit.destinations.len(), 2);
    }

    #[test]
    fn test_spending_limit_empty_destinations_allows_any() {
        let spending_limit = SpendingLimit {
            settings: Pubkey::new_unique(),
            seed: Pubkey::new_unique(),
            account_index: 0,
            mint: Pubkey::default(),
            amount: 1000,
            period: Period::Day,
            remaining_amount: 1000,
            last_reset: 1000000,
            bump: 0,
            signers: vec![Pubkey::new_unique()],
            destinations: vec![], // Empty means any destination
            expiration: 2000000,
        };

        assert!(spending_limit.invariant().is_ok());
        assert!(spending_limit.destinations.is_empty());
    }

    #[test]
    fn test_spending_limit_expiration_timestamp() {
        let current_time = 1000000i64;
        let future_time = current_time + 86400; // 1 day later

        let spending_limit = SpendingLimit {
            settings: Pubkey::new_unique(),
            seed: Pubkey::new_unique(),
            account_index: 0,
            mint: Pubkey::default(),
            amount: 1000,
            period: Period::Day,
            remaining_amount: 1000,
            last_reset: current_time,
            bump: 0,
            signers: vec![Pubkey::new_unique()],
            destinations: vec![],
            expiration: future_time,
        };

        assert!(spending_limit.invariant().is_ok());
        assert!(spending_limit.expiration > spending_limit.last_reset);
    }
}
