use solana_program::pubkey::Pubkey;

use crate::errors::SmartAccountError;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeriodV2 {
    /// The spending limit can only be used once.
    OneTime,
    /// The spending limit is reset every day.
    Daily,
    /// The spending limit is reset every week (7 days).
    Weekly,
    /// The spending limit is reset every month (30 days).
    Monthly,
    /// Custom period in seconds.
    Custom(i64),
}

impl PeriodV2 {
    // 1 (discriminator) + 8 (largest variant `Custom(i64)`).
    pub const INIT_SPACE: usize = 1 + 8;

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

/// Configuration for time-based constraints.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeConstraints {
    /// Optional start timestamp (0 means immediate).
    pub start: i64,
    /// Optional expiration timestamp.
    pub expiration: Option<i64>,
    /// Reset period for the spending limit.
    pub period: PeriodV2,
    /// Whether unused allowances accumulate across periods.
    pub accumulate_unused: bool,
}

impl TimeConstraints {
    // 8 (start) + 1 + 8 (Option<i64>) + PeriodV2::INIT_SPACE + 1 (bool)
    pub const INIT_SPACE: usize = 8 + (1 + 8) + PeriodV2::INIT_SPACE + 1;
}

/// Quantity constraints for spending limits.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuantityConstraints {
    pub max_per_period: u64,
    pub max_per_use: u64,
    pub enforce_exact_quantity: bool,
}

impl QuantityConstraints {
    pub const INIT_SPACE: usize = 8 + 8 + 1;
}

/// Usage tracking for resource consumption.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsageState {
    pub remaining_in_period: u64,
    pub last_reset: i64,
}

impl UsageState {
    pub const INIT_SPACE: usize = 8 + 8;
}

/// Shared spending limit structure that combines timing, quantity, usage, and mint.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpendingLimitV2 {
    pub mint: Pubkey,
    pub time_constraints: TimeConstraints,
    pub quantity_constraints: QuantityConstraints,
    pub usage: UsageState,
}

impl SpendingLimitV2 {
    pub const INIT_SPACE: usize = 32
        + TimeConstraints::INIT_SPACE
        + QuantityConstraints::INIT_SPACE
        + UsageState::INIT_SPACE;

    /// Check if the spending limit is currently active.
    pub fn is_active(&self, current_timestamp: i64) -> Result<(), SmartAccountError> {
        if current_timestamp < self.time_constraints.start {
            return Err(SmartAccountError::SpendingLimitNotActive);
        }

        if let Some(expiration) = self.time_constraints.expiration {
            if current_timestamp > expiration {
                return Err(SmartAccountError::SpendingLimitExpired);
            }
        }

        Ok(())
    }

    pub fn mint(&self) -> Pubkey {
        self.mint
    }

    pub fn remaining_in_period(&self) -> u64 {
        self.usage.remaining_in_period
    }

    pub fn check_amount(&self, amount: u64) -> Result<(), SmartAccountError> {
        if amount > self.usage.remaining_in_period {
            return Err(SmartAccountError::SpendingLimitInsufficientRemainingAmount);
        }
        if self.quantity_constraints.max_per_use > 0
            && amount > self.quantity_constraints.max_per_use
        {
            return Err(SmartAccountError::SpendingLimitViolatesMaxPerUseConstraint);
        }
        if self.quantity_constraints.enforce_exact_quantity
            && amount != self.quantity_constraints.max_per_use
        {
            return Err(SmartAccountError::SpendingLimitViolatesExactQuantityConstraint);
        }

        Ok(())
    }

    pub fn decrement(&mut self, amount: u64) {
        self.usage.remaining_in_period =
            self.usage.remaining_in_period.checked_sub(amount).unwrap();
    }

    /// Reset amounts if period boundary has been crossed.
    pub fn reset_if_needed(&mut self, current_timestamp: i64) {
        if let Some(reset_period) = self.time_constraints.period.to_seconds() {
            if self.is_active(current_timestamp).is_err() {
                return;
            }

            let passed_since_last_reset = current_timestamp
                .checked_sub(self.usage.last_reset)
                .unwrap();

            if passed_since_last_reset > reset_period {
                let periods_passed = passed_since_last_reset.checked_div(reset_period).unwrap();

                self.usage.last_reset = self
                    .usage
                    .last_reset
                    .checked_add(periods_passed.checked_mul(reset_period).unwrap())
                    .unwrap();

                if self.time_constraints.accumulate_unused {
                    let additional_amount = self
                        .quantity_constraints
                        .max_per_period
                        .saturating_mul(periods_passed as u64);
                    self.usage.remaining_in_period = self
                        .usage
                        .remaining_in_period
                        .saturating_add(additional_amount);
                } else {
                    self.usage.remaining_in_period = self.quantity_constraints.max_per_period;
                }
            }
        }
    }

    pub fn invariant(&self) -> Result<(), SmartAccountError> {
        if self.quantity_constraints.max_per_period == 0 {
            return Err(SmartAccountError::SpendingLimitInvariantMaxPerPeriodZero);
        }

        if self.time_constraints.start < 0 {
            return Err(SmartAccountError::SpendingLimitInvariantStartTimePositive);
        }

        if let Some(expiration) = self.time_constraints.expiration {
            if expiration <= self.time_constraints.start {
                return Err(SmartAccountError::SpendingLimitInvariantExpirationSmallerThanStart);
            }
        }

        if self.time_constraints.accumulate_unused {
            if self.time_constraints.period == PeriodV2::OneTime {
                return Err(
                    SmartAccountError::SpendingLimitInvariantOneTimePeriodCannotHaveOverflowEnabled,
                );
            }
            if self.time_constraints.expiration.is_none() {
                return Err(
                    SmartAccountError::SpendingLimitInvariantOverflowEnabledMustHaveExpiration,
                );
            }

            let total_time =
                self.time_constraints.expiration.unwrap() - self.time_constraints.start;
            let period_seconds = self.time_constraints.period.to_seconds().unwrap();
            let total_periods = total_time.checked_div(period_seconds).unwrap() as u64;
            let max_amount = match total_time % period_seconds {
                0 => total_periods
                    .checked_mul(self.quantity_constraints.max_per_period)
                    .unwrap(),
                _ => (total_periods.checked_add(1).unwrap())
                    .checked_mul(self.quantity_constraints.max_per_period)
                    .unwrap(),
            };
            if self.usage.remaining_in_period > max_amount {
                return Err(
                    SmartAccountError::SpendingLimitInvariantOverflowRemainingAmountGreaterThanMaxAmount,
                );
            }
        } else if self.usage.remaining_in_period > self.quantity_constraints.max_per_period {
            return Err(
                SmartAccountError::SpendingLimitInvariantRemainingAmountGreaterThanMaxPerPeriod,
            );
        }

        if self.quantity_constraints.enforce_exact_quantity
            && self.quantity_constraints.max_per_use == 0
        {
            return Err(SmartAccountError::SpendingLimitInvariantExactQuantityMaxPerUseZero);
        }

        if self.quantity_constraints.max_per_use > 0
            && self.quantity_constraints.max_per_use > self.quantity_constraints.max_per_period
        {
            return Err(SmartAccountError::SpendingLimitInvariantMaxPerUseGreaterThanMaxPerPeriod);
        }

        if let PeriodV2::Custom(seconds) = self.time_constraints.period {
            if seconds <= 0 {
                return Err(SmartAccountError::SpendingLimitInvariantCustomPeriodNegative);
            }
        }

        if let Some(expiration) = self.time_constraints.expiration {
            if self.usage.last_reset < self.time_constraints.start
                || self.usage.last_reset > expiration
            {
                return Err(SmartAccountError::SpendingLimitInvariantLastResetOutOfBounds);
            }
        } else if self.usage.last_reset < self.time_constraints.start {
            return Err(SmartAccountError::SpendingLimitInvariantLastResetSmallerThanStart);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let now = 216_000;
        let one_and_a_half_days_ago = now - 129_600;
        let mut policy = SpendingLimitV2 {
            mint: Pubkey::default(),
            time_constraints: make_time_constraints(PeriodV2::Daily, false, 0, None),
            quantity_constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(50, one_and_a_half_days_ago),
        };
        policy.reset_if_needed(now);
        assert_eq!(policy.usage.remaining_in_period, 100);
    }

    #[test]
    fn test_reset_amount_accumulate_unused() {
        let now = 216_000;
        let one_and_a_half_days_ago = now - 129_600;
        let mut policy = SpendingLimitV2 {
            mint: Pubkey::default(),
            time_constraints: make_time_constraints(PeriodV2::Daily, true, 0, None),
            quantity_constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(50, one_and_a_half_days_ago),
        };
        policy.reset_if_needed(now);
        assert_eq!(policy.usage.remaining_in_period, 150);
    }

    #[test]
    fn test_reset_amount_accumulate_unused_2() {
        let now = 216_000;
        let mut policy = SpendingLimitV2 {
            mint: Pubkey::default(),
            time_constraints: make_time_constraints(PeriodV2::Daily, true, 0, None),
            quantity_constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(50, 0),
        };
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
        assert!(policy.is_active(now - 100_000).is_err());
        assert!(policy.is_active(now + 200_000).is_err());
    }
}
