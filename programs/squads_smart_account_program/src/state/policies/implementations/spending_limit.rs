use anchor_lang::{prelude::*, system_program, Ids};
use anchor_spl::token_interface::{self, TokenAccount, TokenInterface, TransferChecked};

use crate::{
    errors::*, get_smart_account_seeds, PolicyTrait, PolicySizeTrait, PolicyPayloadConversionTrait, SEED_PREFIX,
    SEED_SMART_ACCOUNT,
};

/// Enhanced period enum supporting custom durations
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum PeriodV2 {
    /// The spending limit can only be used once
    OneTime,
    /// The spending limit is reset every day
    Day,
    /// The spending limit is reset every week (7 days)
    Week,
    /// The spending limit is reset every month (30 days)
    Month,
    /// Custom period in seconds
    Custom(i64),
}

impl PeriodV2 {
    pub fn to_seconds(&self) -> Option<i64> {
        match self {
            PeriodV2::OneTime => None,
            PeriodV2::Day => Some(24 * 60 * 60),
            PeriodV2::Week => Some(7 * 24 * 60 * 60),
            PeriodV2::Month => Some(30 * 24 * 60 * 60),
            PeriodV2::Custom(seconds) => Some(*seconds),
        }
    }
}

/// Configuration for time-based constraints
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub struct TimeConstraints {
    /// Optional start timestamp (0 means immediate)
    pub start: i64,
    /// Optional expiration timestamp (0 means no expiration)
    pub expiration: Option<i64>,
    /// Reset period for the resource limit
    pub period: PeriodV2,
    /// Whether unused allowances accumulate across periods
    pub accumulate_unused: bool,
}

/// Quantity constraints for resource limits
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub struct QuantityConstraints {
    /// Maximum quantity per period
    pub max_per_period: u64,
    /// Maximum quantity per individual use (0 means no per-use limit)
    pub max_per_use: u64,
    /// Whether to enforce exact quantity matching
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

/// Main spending limit structure using composition
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct SpendingLimitPolicy {
    /// The token mint the spending limit is for.
    /// Pubkey::default() means SOL.
    /// use NATIVE_MINT for Wrapped SOL.
    pub mint: Pubkey,

    /// The source account index.
    pub source_account_index: u8,

    /// Timing configuration
    pub timing: TimeConstraints,

    /// Amount constraints
    pub constraints: QuantityConstraints,

    /// Current usage tracking
    pub usage: UsageState,

    /// The destination addresses the spending limit is allowed to sent funds to.
    /// If empty, funds can be sent to any address.
    pub destinations: Vec<Pubkey>,
}

impl SpendingLimitPolicy {
    pub fn size(destinations_length: usize) -> usize {
        8  + // anchor discriminator
        32 + // mint
        std::mem::size_of::<TimeConstraints>() + // timing
        std::mem::size_of::<QuantityConstraints>() + // constraints
        std::mem::size_of::<UsageState>() + // usage
        4  + // destinations vector length
        destinations_length * 32 + // destinations
        1 // bump
    }

    /// Check if the spending limit is currently active
    pub fn is_active(&self, current_timestamp: i64) -> bool {
        // Check start time
        if current_timestamp < self.timing.start {
            return false;
        }

        // Check expiration
        if self.timing.expiration.is_some() && current_timestamp > self.timing.expiration.unwrap() {
            return false;
        }

        true
    }

    pub fn decrement_amount(&mut self, amount: u64) {
        self.usage.remaining_in_period = self.usage.remaining_in_period.saturating_sub(amount);
    }

    /// Reset amounts if period boundary has been crossed
    pub fn reset_amount_if_needed(&mut self, current_timestamp: i64) {
        // Apply same reset logic as in use_spending_limit.rs lines 161-175
        if let Some(reset_period) = self.timing.period.to_seconds() {
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

                if self.timing.accumulate_unused {
                    // For overflow: add missed periods to current amount
                    // (overflow is only enabled with expiration, so we know it exists)
                    let additional_amount = self
                        .constraints
                        .max_per_period
                        .saturating_mul(periods_passed as u64);
                    self.usage.remaining_in_period = self
                        .usage
                        .remaining_in_period
                        .saturating_add(additional_amount);
                } else {
                    // For non-overflow: reset to full period amount (original behavior)
                    self.usage.remaining_in_period = self.constraints.max_per_period;
                }
            }
        }
    }
}

/// Setup parameters for creating a spending limit
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct SpendingLimitPolicyCreationPayload {
    pub mint: Pubkey,
    pub source_account_index: u8,
    pub time_constraints: TimeConstraints,
    pub quantity_constraints: QuantityConstraints,
    pub destinations: Vec<Pubkey>,
}

impl PolicyPayloadConversionTrait for SpendingLimitPolicyCreationPayload {
    type PolicyState = SpendingLimitPolicy;

    fn to_policy_state(self) -> SpendingLimitPolicy {
        let now = Clock::get().unwrap().unix_timestamp;
        SpendingLimitPolicy {
            mint: self.mint,
            source_account_index: self.source_account_index,
            timing: self.time_constraints,
            constraints: self.quantity_constraints,
            usage: UsageState {
                remaining_in_period: self.quantity_constraints.max_per_period,
                last_reset: now,
            },
            destinations: self.destinations,
        }
    }
}

impl PolicySizeTrait for SpendingLimitPolicyCreationPayload {
    fn creation_payload_size(&self) -> usize {
        32 + // mint
        1 + // source_account_index
        TimeConstraints::INIT_SPACE + // time_constraints
        QuantityConstraints::INIT_SPACE + // quantity_constraints
        4 + self.destinations.len() * 32 // destinations vec
    }

    fn policy_state_size(&self) -> usize {
        32 + // mint
        1 + // source_account_index
        TimeConstraints::INIT_SPACE + // time_constraints
        QuantityConstraints::INIT_SPACE + // quantity_constraints
        UsageState::INIT_SPACE + // usage
        4 + self.destinations.len() * 32 // destinations vec
    }
}

/// Payload for using a spending limit policy.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct SpendingLimitPayload {
    pub amount: u64,
    pub destination: Pubkey,
    pub decimals: u8,
}

pub struct SpendingLimitExecutionArgs {
    pub settings_key: Pubkey,
}

impl PolicyTrait for SpendingLimitPolicy {
    type PolicyState = Self;
    type CreationPayload = SpendingLimitPolicyCreationPayload;
    type UsagePayload = SpendingLimitPayload;
    type ExecutionArgs = SpendingLimitExecutionArgs;

    fn invariant(&self) -> Result<()> {
        // Amount per period must be non-zero
        require_neq!(
            self.constraints.max_per_period,
            0,
            SmartAccountError::SpendingLimitInvalidAmount
        );

        // If start time is set, it must be positive
        require!(
            self.timing.start > 0,
            SmartAccountError::SpendingLimitInvalidCadenceConfiguration
        );

        // If expiration is set, it must be positive
        if self.timing.expiration.is_some() {
            require!(
                self.timing.expiration.unwrap() > 0,
                SmartAccountError::SpendingLimitInvalidCadenceConfiguration
            );
        }

        // If both start and expiration are set, start must be before expiration
        if self.timing.expiration.is_some() {
            require!(
                self.timing.start < self.timing.expiration.unwrap(),
                SmartAccountError::SpendingLimitInvalidCadenceConfiguration
            );
        }

        // If overflow is enabled, must have both start and expiration
        if self.timing.accumulate_unused {
            require!(
                self.timing.expiration.is_some(),
                SmartAccountError::SpendingLimitInvalidCadenceConfiguration
            );
        }

        // If exact amount is enforced, per-use amount must be set and non-zero
        if self.constraints.enforce_exact_quantity {
            require!(
                self.constraints.max_per_use > 0,
                SmartAccountError::SpendingLimitInvalidAmount
            );
        }

        // If per-use amount is set, it cannot exceed per-period amount
        if self.constraints.max_per_use > 0 {
            require!(
                self.constraints.max_per_use <= self.constraints.max_per_period,
                SmartAccountError::SpendingLimitInvalidAmount
            );
        }

        // OneTime period cannot have overflow enabled
        if self.timing.period == PeriodV2::OneTime {
            require!(
                !self.timing.accumulate_unused,
                SmartAccountError::SpendingLimitInvalidCadenceConfiguration
            );
        }

        // Custom period must have positive duration
        if let PeriodV2::Custom(seconds) = self.timing.period {
            require!(
                seconds > 0,
                SmartAccountError::SpendingLimitInvalidCadenceConfiguration
            );
        }

        // Usage tracking invariants
        if !self.timing.accumulate_unused {
            require!(
                self.usage.remaining_in_period <= self.constraints.max_per_period,
                SmartAccountError::SpendingLimitInvalidAmount
            );
        }

        // Last reset must be positive
        require!(
            self.usage.last_reset > 0,
            SmartAccountError::SpendingLimitInvalidCadenceConfiguration
        );

        Ok(())
    }

    fn validate_payload(&self, payload: &Self::UsagePayload) -> Result<()> {
        // Check that the destination is in the list of allowed destinations
        require!(
            self.destinations.contains(&payload.destination),
            SmartAccountError::InvalidDestination
        );

        Ok(())
    }

    fn execute_payload<'info>(
        &mut self,
        args: Self::ExecutionArgs,
        payload: &Self::UsagePayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        let current_timestamp = Clock::get()?.unix_timestamp;

        // Reset the period & amount
        self.reset_amount_if_needed(current_timestamp);

        // Check that the amount is less than the remaining amount
        require!(
            payload.amount <= self.usage.remaining_in_period,
            SmartAccountError::SpendingLimitInvalidAmount
        );

        // Validate the accounts
        let validated_accounts = self.validate_accounts(&args.settings_key, &payload, accounts)?;

        // Execute the payload
        match validated_accounts {
            ValidatedAccounts::NativeTransfer {
                source_account_info,
                source_account_bump,
                destination_account_info,
                system_program,
            } => {
                // Transfer SOL
                anchor_lang::system_program::transfer(
                    CpiContext::new_with_signer(
                        system_program.to_account_info(),
                        anchor_lang::system_program::Transfer {
                            from: source_account_info.clone(),
                            to: destination_account_info.clone(),
                        },
                        &[&[
                            SEED_PREFIX,
                            args.settings_key.as_ref(),
                            SEED_SMART_ACCOUNT,
                            &self.source_account_index.to_le_bytes(),
                            &[source_account_bump],
                        ]],
                    ),
                    payload.amount,
                )?
            }
            ValidatedAccounts::TokenTransfer {
                source_account_info,
                source_account_bump,
                source_token_account_info,
                destination_token_account_info,
                mint,
                token_program,
            } => {
                // Transfer SPL token
                token_interface::transfer_checked(
                    CpiContext::new_with_signer(
                        token_program.to_account_info(),
                        TransferChecked {
                            from: source_token_account_info.to_account_info(),
                            mint: mint.to_account_info(),
                            to: destination_token_account_info.to_account_info(),
                            authority: source_account_info.clone(),
                        },
                        &[&[
                            SEED_PREFIX,
                            args.settings_key.as_ref(),
                            SEED_SMART_ACCOUNT,
                            &self.source_account_index.to_le_bytes(),
                            &[source_account_bump],
                        ]],
                    ),
                    payload.amount,
                    payload.decimals,
                )?;
            }
        }

        // Decrement the amount
        self.decrement_amount(payload.amount);

        // Invariant check
        self.invariant()?;

        Ok(())
    }
}

enum ValidatedAccounts<'info> {
    NativeTransfer {
        source_account_info: &'info AccountInfo<'info>,
        source_account_bump: u8,
        destination_account_info: &'info AccountInfo<'info>,
        system_program: &'info AccountInfo<'info>,
    },
    TokenTransfer {
        source_account_info: &'info AccountInfo<'info>,
        source_account_bump: u8,
        source_token_account_info: &'info AccountInfo<'info>,
        destination_token_account_info: &'info AccountInfo<'info>,
        mint: &'info AccountInfo<'info>,
        token_program: &'info AccountInfo<'info>,
    },
}
impl SpendingLimitPolicy {
    /// Validates the accounts passed in and returns a struct with the accounts
    fn validate_accounts<'info>(
        &self,
        settings_key: &Pubkey,
        args: &SpendingLimitPayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<ValidatedAccounts<'info>> {
        // Derive source account key
        let source_account_index_bytes = self.source_account_index.to_le_bytes();
        let source_account_seeds =
            get_smart_account_seeds(settings_key, &source_account_index_bytes);

        // Derive source and destination account keys
        let (source_account_key, source_account_bump) =
            Pubkey::find_program_address(source_account_seeds.as_slice(), &crate::ID);

        // Mint specific logic
        match self.mint {
            // Native SOL transfer
            mint if mint == Pubkey::default() => {
                // Parse out the accounts
                let (source_account_info, destination_account_info, system_program) = if let [source_account_info, destination_account_info, system_program, _remaining @ ..] =
                    accounts
                {
                    (
                        source_account_info,
                        destination_account_info,
                        system_program,
                    )
                } else {
                    return err!(SmartAccountError::InvalidNumberOfAccounts);
                };
                // Check that the source account is the same as the source account info
                require!(
                    source_account_key == source_account_info.key(),
                    SmartAccountError::InvalidAccount
                );
                // Check that the destination account is the same as the destination account info
                require!(
                    args.destination == destination_account_info.key(),
                    SmartAccountError::InvalidAccount
                );
                // Check the system program
                require!(
                    system_program.key() == system_program::ID,
                    SmartAccountError::InvalidAccount
                );

                // Sanity check for the decimals. Similar to the one in token_interface::transfer_checked.
                require!(args.decimals == 9, SmartAccountError::DecimalsMismatch);

                Ok(ValidatedAccounts::NativeTransfer {
                    source_account_info,
                    source_account_bump,
                    destination_account_info,
                    system_program,
                })
            }
            // Token transfer
            _ => {
                // Parse out the accounts
                let (
                    source_account_info,
                    source_token_account_info,
                    destination_token_account_info,
                    mint,
                    token_program,
                ) = if let [source_account_info, source_token_account_info, destination_token_account_info, mint, token_program, _remaining @ ..] =
                    accounts
                {
                    (
                        source_account_info,
                        source_token_account_info,
                        destination_token_account_info,
                        mint,
                        token_program,
                    )
                } else {
                    return err!(SmartAccountError::InvalidNumberOfAccounts);
                };
                // Deserialize the source and destination token accounts. Either
                // T22 or TokenKeg accounts
                let source_token_account =
                    InterfaceAccount::<'info, TokenAccount>::try_from(source_token_account_info)?;
                let destination_token_account =
                    InterfaceAccount::<TokenAccount>::try_from(destination_token_account_info)?;
                // Check the mint against the policy state
                require_eq!(self.mint, mint.key());

                // Assert the ownership and mint of the token accounts
                require!(
                    source_token_account.owner == source_account_key
                        && source_token_account.mint == self.mint,
                    SmartAccountError::InvalidAccount
                );
                require!(
                    destination_token_account.owner == args.destination
                        && destination_token_account.mint == self.mint,
                    SmartAccountError::InvalidAccount
                );
                // Check the token program
                require_eq!(TokenInterface::ids().contains(&token_program.key()), true);

                Ok(ValidatedAccounts::TokenTransfer {
                    source_account_info,
                    source_account_bump,
                    source_token_account_info,
                    destination_token_account_info,
                    mint,
                    token_program,
                })
            }
        }
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
        let mut policy = SpendingLimitPolicy {
            mint: Pubkey::default(),
            source_account_index: 0,
            timing: make_time_constraints(PeriodV2::Day, false, 0, None),
            constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(50, one_and_a_half_days_ago), // last reset was 1 day ago
            destinations: vec![],
        };
        // Should reset to max_per_period
        policy.reset_amount_if_needed(now);
        assert_eq!(policy.usage.remaining_in_period, 100);
    }

    #[test]
    fn test_reset_amount_accumulate_unused() {
        // 2.5 days in seconds
        let now = 216_000;
        let one_and_a_half_days_ago = now - 129_600;
        let mut policy = SpendingLimitPolicy {
            mint: Pubkey::default(),
            source_account_index: 0,
            timing: make_time_constraints(PeriodV2::Day, true, 0, None),
            constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(50, one_and_a_half_days_ago), // last reset was 1.5 days ago
            destinations: vec![],
        };
        // Should reset to max_per_period
        policy.reset_amount_if_needed(now);
        assert_eq!(policy.usage.remaining_in_period, 150);
    }

    #[test]
    fn test_reset_amount_accumulate_unused_2() {
        // 2.5 days in seconds
        let now = 216_000;
        let mut policy = SpendingLimitPolicy {
            mint: Pubkey::default(),
            source_account_index: 0,
            timing: make_time_constraints(PeriodV2::Day, true, 0, None),
            constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(50, 0), // last reset was 1.5 days ago
            destinations: vec![],
        };
        // Should reset to max_per_period
        policy.reset_amount_if_needed(now);
        assert_eq!(policy.usage.remaining_in_period, 250);
    }

    #[test]
    fn test_decrement_amount() {
        let mut policy = SpendingLimitPolicy {
            mint: Pubkey::default(),
            source_account_index: 0,
            timing: make_time_constraints(PeriodV2::Day, false, 1_000_000, None),
            constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(100, 1_000_000),
            destinations: vec![],
        };
        policy.decrement_amount(30);
        assert_eq!(policy.usage.remaining_in_period, 70);
    }

    #[test]
    fn test_is_active() {
        let now = 1_000_000;
        let policy = SpendingLimitPolicy {
            mint: Pubkey::default(),
            source_account_index: 0,
            timing: make_time_constraints(PeriodV2::Day, false, now - 10, Some(now + 100)),
            constraints: make_quantity_constraints(100, 0, false),
            usage: make_usage_state(100, now - 10),
            destinations: vec![],
        };
        assert!(policy.is_active(now));
        assert!(!policy.is_active(now - 100_000)); // before start
        assert!(!policy.is_active(now + 200_000)); // after expiration
    }
}
