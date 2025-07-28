use anchor_lang::{prelude::*, system_program, Ids};
use anchor_spl::token_interface::{self, TokenAccount, TokenInterface, TransferChecked};

use crate::{
    errors::*, get_smart_account_seeds, state::policies::utils::{QuantityConstraints, SpendingLimitV2, TimeConstraints, UsageState}, PolicyExecutionContext, PolicyPayloadConversionTrait, PolicySizeTrait, PolicyTrait, SEED_PREFIX, SEED_SMART_ACCOUNT
};

/// == SpendingLimitPolicy ==
/// This policy allows for the transfer of SOL and SPL tokens between
/// a source account and a set of destination accounts.
///
/// The policy is defined by a spending limit configuration and a source account index.
/// The spending limit configuration includes a mint, time constraints, quantity constraints,
/// and usage state.
///===============================================


// =============================================================================
// CORE POLICY STRUCTURES
// =============================================================================

/// Main spending limit policy structure
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct SpendingLimitPolicy {
    /// The source account index
    pub source_account_index: u8,
    /// The destination addresses the spending limit is allowed to send funds to
    /// If empty, funds can be sent to any address
    pub destinations: Vec<Pubkey>,
    /// Spending limit configuration (timing, constraints, usage, mint)
    pub spending_limit: SpendingLimitV2,
}

// =============================================================================
// CREATION PAYLOAD TYPES
// =============================================================================

/// Setup parameters for creating a spending limit
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct SpendingLimitPolicyCreationPayload {
    pub mint: Pubkey,
    pub source_account_index: u8,
    pub time_constraints: TimeConstraints,
    pub quantity_constraints: QuantityConstraints,
    /// Optionally this can be submitted to update a spending limit policy
    /// Cannot be Some() if accumulate_unused is true, to avoid invariant behavior
    pub usage_state: Option<UsageState>,
    pub destinations: Vec<Pubkey>,
}

// =============================================================================
// EXECUTION PAYLOAD TYPES
// =============================================================================

/// Payload for using a spending limit policy
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct SpendingLimitPayload {
    pub amount: u64,
    pub destination: Pubkey,
    pub decimals: u8,
}

pub struct SpendingLimitExecutionArgs {
    pub settings_key: Pubkey,
}

/// Validated account information for different transfer types
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

// =============================================================================
// PAYLOAD CONVERSION IMPLEMENTATIONS
// =============================================================================

impl PolicyPayloadConversionTrait for SpendingLimitPolicyCreationPayload {
    type PolicyState = SpendingLimitPolicy;

    /// Convert creation payload to policy state
    /// Used by Settings.modify_with_action() to instantiate policy state
    fn to_policy_state(self) -> Result<SpendingLimitPolicy> {
        let now = Clock::get().unwrap().unix_timestamp;
        // Sort the destinations
        let mut destinations = self.destinations;
        destinations.sort_by_key(|d| d.to_bytes());

        // Modify time constraints to start at the current timestamp if set to 0
        let mut modified_time_constraints = self.time_constraints;
        if self.time_constraints.start == 0 {
            modified_time_constraints.start = now;
        }

        // Determine usage state based on surrounding constraints
        let usage_state = if let Some(usage_state) = self.usage_state {
            // This is the only invariant that needs to be checked on the arg level
            require!(
                !self.time_constraints.accumulate_unused,
                SmartAccountError::SpendingLimitPolicyInvariantAccumulateUnused
            );
            usage_state
        } else {
            UsageState {
                remaining_in_period: self.quantity_constraints.max_per_period,
                last_reset: modified_time_constraints.start,
            }
        };

        Ok(SpendingLimitPolicy {
            spending_limit: SpendingLimitV2 {
                mint: self.mint,
                time_constraints: self.time_constraints,
                quantity_constraints: self.quantity_constraints,
                usage: usage_state,
            },
            source_account_index: self.source_account_index,
            destinations,
        })
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
        32 + // mint (in SpendingLimitV2)
        TimeConstraints::INIT_SPACE + // time_constraints (in SpendingLimitV2)
        QuantityConstraints::INIT_SPACE + // quantity_constraints (in SpendingLimitV2)
        UsageState::INIT_SPACE + // usage (in SpendingLimitV2)
        1 + // source_account_index
        4 + self.destinations.len() * 32 // destinations vec
    }
}

// =============================================================================
// POLICY TRAIT IMPLEMENTATION
// =============================================================================

impl PolicyTrait for SpendingLimitPolicy {
    type PolicyState = Self;
    type CreationPayload = SpendingLimitPolicyCreationPayload;
    type UsagePayload = SpendingLimitPayload;
    type ExecutionArgs = SpendingLimitExecutionArgs;

    /// Validate policy invariants - no duplicate destinations and valid spending limit
    fn invariant(&self) -> Result<()> {
        // Check that the destinations are not duplicated (assumes sorted destinations)
        let has_duplicates = self.destinations.windows(2).any(|w| w[0] == w[1]);
        require!(
            !has_duplicates,
            SmartAccountError::SpendingLimitPolicyInvariantDuplicateDestinations
        );

        // Check the spending limit invariant
        self.spending_limit.invariant()?;
        Ok(())
    }

    /// Validate that the destination is allowed
    fn validate_payload(
        &self,
        // No difference between synchronous and asynchronous execution
        _context: PolicyExecutionContext,
        payload: &Self::UsagePayload,
    ) -> Result<()> {
        // Check that the destination is in the list of allowed destinations
        require!(
            self.destinations.contains(&payload.destination),
            SmartAccountError::InvalidDestination
        );

        Ok(())
    }

    /// Execute the spending limit transfer
    fn execute_payload<'info>(
        &mut self,
        args: Self::ExecutionArgs,
        payload: &Self::UsagePayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        let current_timestamp = Clock::get()?.unix_timestamp;

        // Check that the spending limit is active
        self.spending_limit.is_active(current_timestamp)?;

        // Reset the period & amount
        self.spending_limit.reset_if_needed(current_timestamp);

        // Check that the amount complies with the spending limit
        self.spending_limit.check_amount(payload.amount)?;

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
        self.spending_limit.decrement(payload.amount);

        // Invariant check
        self.invariant()?;

        Ok(())
    }
}

// =============================================================================
// ACCOUNT VALIDATION
// =============================================================================

impl SpendingLimitPolicy {
    /// Validate the accounts needed for transfer execution
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
        match self.spending_limit.mint {
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
                require_eq!(self.spending_limit.mint, mint.key());

                // Assert the ownership and mint of the token accounts
                require!(
                    source_token_account.owner == source_account_key
                        && source_token_account.mint == self.spending_limit.mint,
                    SmartAccountError::InvalidAccount
                );
                require!(
                    destination_token_account.owner == args.destination
                        && destination_token_account.mint == self.spending_limit.mint,
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
