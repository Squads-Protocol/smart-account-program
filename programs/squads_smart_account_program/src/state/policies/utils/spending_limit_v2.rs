use anchor_lang::prelude::*;

use crate::errors::SmartAccountError;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum PeriodV2 {
    /// The spending limit can only be used once
    OneTime,
    /// The spending limit is reset every day
    Daily,
    /// The spending limit is reset every week (7 days)
    Weekly,
    /// The spending limit is reset every month (30 days)
    Monthly,
    /// Custom period in seconds
    Custom(i64),
}

impl PeriodV2 {
    pub fn to_seconds(&self) -> Option<i64> {
        match self {
            PeriodV2::OneTime => None,
            PeriodV2::Daily => Some(24 * 60 * 60),
            PeriodV2::Weekly => Some(7 * 24 * 60 * 60),
            PeriodV2::Monthly => Some(30 * 24 * 60 * 60),
            PeriodV2::Custom(seconds) => Some(*seconds),
        }
    }
}

/// Configuration for time-based constraints
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub struct TimeConstraints {
    /// Optional start timestamp (0 means immediate)
    pub start: i64,
    /// Optional expiration timestamp
    pub expiration: Option<i64>,
    /// Reset period for the spending limit
    pub period: PeriodV2,
    /// Whether unused allowances accumulate across periods
    pub accumulate_unused: bool,
}

/// Quantity constraints for spending limits
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub struct QuantityConstraints {
    /// Maximum quantity per period
    pub max_per_period: u64,
    /// Maximum quantity per individual use (0 means no per-use limit)
    pub max_per_use: u64,
    /// Whether to enforce exact quantity matching on max per use.
    pub enforce_exact_quantity: bool,
}

/// Usage tracking for resource consumption
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub struct UsageState {
    /// Remaining quantity in current period
    pub remaining_in_period: u64,
    /// Unix timestamp of last reset
    pub last_reset: i64,
}

/// Shared spending limit structure that combines timing, quantity, usage, and mint
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq, InitSpace)]
pub struct SpendingLimitV2 {
    /// The token mint the spending limit is for.
    /// Pubkey::default() means SOL.
    /// use NATIVE_MINT for Wrapped SOL.
    pub mint: Pubkey,

    /// Timing configuration
    pub time_constraints: TimeConstraints,

    /// Amount constraints
    pub quantity_constraints: QuantityConstraints,

    /// Current usage tracking
    pub usage: UsageState,
}

impl SpendingLimitV2 {
    /// Check if the spending limit is currently active
    pub fn is_active(&self, current_timestamp: i64) -> Result<()> {
        // Check start time
        if current_timestamp < self.time_constraints.start {
            return err!(SmartAccountError::SpendingLimitNotActive);
        }

        // Check expiration
        if let Some(expiration) = self.time_constraints.expiration {
            if current_timestamp > expiration {
                return err!(SmartAccountError::SpendingLimitExpired);
            }
        }

        Ok(())
    }

    // Returns the mint of the spending limit
    pub fn mint(&self) -> Pubkey {
        self.mint
    }

    // Returns the remaining amount in the period for the spending limit
    pub fn remaining_in_period(&self) -> u64 {
        self.usage.remaining_in_period
    }

    /// Check that the amount is less than the remaining amount, and if it complies with the quantity constraints
    pub fn check_amount(&self, amount: u64) -> Result<()> {
        // Remaining amount constraint
        if amount > self.usage.remaining_in_period {
            return err!(SmartAccountError::SpendingLimitInsufficientRemainingAmount);
        }
        // Max per use constraint
        if self.quantity_constraints.max_per_use > 0 {
            require!(
                amount <= self.quantity_constraints.max_per_use,
                SmartAccountError::SpendingLimitViolatesMaxPerUseConstraint
            );
        }
        // Exact amount constraint
        if self.quantity_constraints.enforce_exact_quantity {
            // Exact max per use constraint
            require_eq!(
                amount,
                self.quantity_constraints.max_per_use,
                SmartAccountError::SpendingLimitViolatesExactQuantityConstraint
            );
        }

        Ok(())
    }

    pub fn decrement(&mut self, amount: u64) {
        self.usage.remaining_in_period =
            self.usage.remaining_in_period.checked_sub(amount).unwrap();
    }

    /// Reset amounts if period boundary has been crossed
    pub fn reset_if_needed(&mut self, current_timestamp: i64) {
        // Reset logic for spending limits
        if let Some(reset_period) = self.time_constraints.period.to_seconds() {
            // Check that the spending limit is active
            if self.is_active(current_timestamp).is_err() {
                return;
            }

            let passed_since_last_reset = current_timestamp
                .checked_sub(self.usage.last_reset)
                .unwrap();

            if passed_since_last_reset > reset_period {
                let periods_passed = passed_since_last_reset.checked_div(reset_period).unwrap();

                // Update last_reset: last_reset = last_reset + periods_passed * reset_period
                self.usage.last_reset = self
                    .usage
                    .last_reset
                    .checked_add(periods_passed.checked_mul(reset_period).unwrap())
                    .unwrap();

                if self.time_constraints.accumulate_unused {
                    // For overflow: add missed periods to current amount
                    // (overflow is only enabled with expiration, so we know it exists)
                    let additional_amount = self
                        .quantity_constraints
                        .max_per_period
                        .saturating_mul(periods_passed as u64);
                    self.usage.remaining_in_period = self
                        .usage
                        .remaining_in_period
                        .saturating_add(additional_amount);
                } else {
                    // For non-overflow: reset to full period amount (original behavior)
                    self.usage.remaining_in_period = self.quantity_constraints.max_per_period;
                }
            }
        }
    }

    pub fn invariant(&self) -> Result<()> {
        // Amount per period must be non-zero
        require_neq!(
            self.quantity_constraints.max_per_period,
            0,
            SmartAccountError::SpendingLimitInvariantMaxPerPeriodZero
        );

        // If start time is set, it must be positive
        require!(
            self.time_constraints.start >= 0,
            SmartAccountError::SpendingLimitInvariantStartTimePositive
        );

        // If expiration is set, it must be positive
        if self.time_constraints.expiration.is_some() {
            // Since start is positive, expiration must be greater than start,
            // we can skip the check for expiration being positive.
            require!(
                self.time_constraints.expiration.unwrap() > self.time_constraints.start,
                SmartAccountError::SpendingLimitInvariantExpirationSmallerThanStart
            );
        }

        // If overflow is enabled, must have expiration. This is to prevent
        // footguns
        if self.time_constraints.accumulate_unused {
            // OneTime period cannot have overflow enabled
            require!(
                self.time_constraints.period != PeriodV2::OneTime,
                SmartAccountError::SpendingLimitInvariantOneTimePeriodCannotHaveOverflowEnabled
            );
            require!(
                self.time_constraints.expiration.is_some(),
                SmartAccountError::SpendingLimitInvariantOverflowEnabledMustHaveExpiration
            );
            // Remaining amount must always be less than expiration - start /
            // period + 1 * max per period
            let total_time =
                self.time_constraints.expiration.unwrap() - self.time_constraints.start;
            let total_periods = total_time
                .checked_div(self.time_constraints.period.to_seconds().unwrap())
                .unwrap() as u64;
            // Total amount based on number of periods within start & expiration
            let max_amount = match total_time % self.time_constraints.period.to_seconds().unwrap() {
                // Start & Expiration are divisible by period, so we can use the
                // total number of periods to calculate the max amount.
                0 => total_periods
                    .checked_mul(self.quantity_constraints.max_per_period)
                    .unwrap(),
                // Start & Expiration are divisible by period with a remainder, so we need to
                // add an extra period to the total number of periods.
                _ => (total_periods.checked_add(1).unwrap())
                    .checked_mul(self.quantity_constraints.max_per_period)
                    .unwrap(),
            };
            // Remaining amount must always be less than max amount
            require!(
                self.usage.remaining_in_period <= max_amount,
                SmartAccountError::SpendingLimitInvariantOverflowRemainingAmountGreaterThanMaxAmount
            );
        } else {
            // If overflow is disabled, remaining in period must be less than or equal to max per period
            require!(
                self.usage.remaining_in_period <= self.quantity_constraints.max_per_period,
                SmartAccountError::SpendingLimitInvariantRemainingAmountGreaterThanMaxPerPeriod
            );
        }

        // If exact amount is enforced, per-use amount must be set and non-zero
        if self.quantity_constraints.enforce_exact_quantity {
            require!(
                self.quantity_constraints.max_per_use > 0,
                SmartAccountError::SpendingLimitInvariantExactQuantityMaxPerUseZero
            );
        }

        // If per-use amount is set, it cannot exceed per-period amount
        if self.quantity_constraints.max_per_use > 0 {
            require!(
                self.quantity_constraints.max_per_use <= self.quantity_constraints.max_per_period,
                SmartAccountError::SpendingLimitInvariantMaxPerUseGreaterThanMaxPerPeriod
            );
        }

        // Custom period must have positive duration
        if let PeriodV2::Custom(seconds) = self.time_constraints.period {
            require!(
                seconds > 0,
                SmartAccountError::SpendingLimitInvariantCustomPeriodNegative
            );
        }

        // Last reset must be between start and expiration
        if let Some(expiration) = self.time_constraints.expiration {
            require!(
                self.usage.last_reset >= self.time_constraints.start
                    && self.usage.last_reset <= expiration,
                SmartAccountError::SpendingLimitInvariantLastResetOutOfBounds
            );
        } else {
            require!(
                self.usage.last_reset >= self.time_constraints.start,
                SmartAccountError::SpendingLimitInvariantLastResetSmallerThanStart
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::pubkey::Pubkey;

    fn make_time_constraints(
        period: PeriodV2,
        accumulate_unused: bool,
        start: i64,
        expiration: Option<i64>,
    ) -> TimeConstraints {
        TimeConstraints {
            start,
            expiration,
            period,
            accumulate_unused,
        }
    }

    fn make_quantity_constraints(
        max_per_period: u64,
        max_per_use: u64,
        enforce_exact_quantity: bool,
    ) -> QuantityConstraints {
        QuantityConstraints {
            max_per_period,
            max_per_use,
            enforce_exact_quantity,
        }
    }

    fn make_usage_state(remaining: u64, last_reset: i64) -> UsageState {
        UsageState {
            remaining_in_period: remaining,
            last_reset,
        }
    }

    #[test]
    fn test_reset_amount_non_accumulate_unused() {
        // 2.5 days in seconds
        let now = 216_000;
        let one_and_a_half_days_ago = now - 129_600;
        let mut policy = SpendingLimitV2 {
            mint: Pubkey::default(),
            time_constraints: make_time_constraints(PeriodV2::Daily, false, 0, None),
            quantity_constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(50, one_and_a_half_days_ago), // last reset was 1 day ago
        };
        // Should reset to max_per_period
        policy.reset_if_needed(now);
        assert_eq!(policy.usage.remaining_in_period, 100);
    }

    #[test]
    fn test_reset_amount_accumulate_unused() {
        // 2.5 days in seconds
        let now = 216_000;
        let one_and_a_half_days_ago = now - 129_600;
        let mut policy = SpendingLimitV2 {
            mint: Pubkey::default(),
            time_constraints: make_time_constraints(PeriodV2::Daily, true, 0, None),
            quantity_constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(50, one_and_a_half_days_ago), // last reset was 1.5 days ago
        };
        // Should reset to max_per_period
        policy.reset_if_needed(now);
        assert_eq!(policy.usage.remaining_in_period, 150);
    }

    #[test]
    fn test_reset_amount_accumulate_unused_2() {
        // 2.5 days in seconds
        let now = 216_000;
        let mut policy = SpendingLimitV2 {
            mint: Pubkey::default(),
            time_constraints: make_time_constraints(PeriodV2::Daily, true, 0, None),
            quantity_constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(50, 0), // last reset was 1.5 days ago
        };
        // Should reset to max_per_period
        policy.reset_if_needed(now);
        assert_eq!(policy.usage.remaining_in_period, 250);
    }

    #[test]
    fn test_decrement_amount() {
        let mut policy = SpendingLimitV2 {
            mint: Pubkey::default(),
            time_constraints: make_time_constraints(PeriodV2::Daily, false, 1_000_000, None),
            quantity_constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(100, 1_000_000),
        };
        policy.decrement(30);
        assert_eq!(policy.usage.remaining_in_period, 70);
    }

    #[test]
    fn test_is_active() {
        let now = 1_000_000;
        let policy = SpendingLimitV2 {
            mint: Pubkey::default(),
            time_constraints: make_time_constraints(
                PeriodV2::Daily,
                false,
                now - 10,
                Some(now + 100),
            ),
            quantity_constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(100, now - 10),
        };
        assert!(policy.is_active(now).is_ok());
        assert!(policy.is_active(now - 100_000).is_err()); // before start
        assert!(policy.is_active(now + 200_000).is_err()); // after expiration
    }
}
