//! `SpendingLimitPolicy` — types re-exported from the types crate. The
//! creation-payload conversion uses `Clock::get` and therefore can't be an
//! `impl PolicyPayloadConversionTrait` (both trait and payload are foreign);
//! we expose a free function `spending_limit_creation_to_policy_state`
//! instead, called from `Settings::modify_with_action`.

use anchor_lang::{prelude::*, system_program, Ids};
use anchor_spl::token_interface::{self, TokenAccount, TokenInterface, TransferChecked};

pub use squads_smart_account_program_types::{
    SpendingLimitExecutionArgs, SpendingLimitPayload, SpendingLimitPolicy,
    SpendingLimitPolicyCreationPayload,
};
use squads_smart_account_program_types::{SpendingLimitV2, UsageState};

use crate::error_conv::ToAnchorResult;
use crate::{
    errors::*,
    get_smart_account_seeds,
    state::policies::policy_core::{PolicyExecutionContext, PolicyTrait},
    SEED_PREFIX, SEED_SMART_ACCOUNT,
};

/// Convert a `SpendingLimitPolicyCreationPayload` into a
/// `SpendingLimitPolicy`, using `Clock::get` for start-timestamp fallback.
pub fn spending_limit_creation_to_policy_state(
    payload: SpendingLimitPolicyCreationPayload,
) -> Result<SpendingLimitPolicy> {
    let now = Clock::get()?.unix_timestamp;

    let mut destinations = payload.destinations;
    destinations.sort_by_key(|d| d.to_bytes());

    let mut modified_time_constraints = payload.time_constraints;
    if payload.time_constraints.start == 0 {
        modified_time_constraints.start = now;
    }

    let usage_state = if let Some(usage_state) = payload.usage_state {
        require!(
            !payload.time_constraints.accumulate_unused,
            SmartAccountError::SpendingLimitPolicyInvariantAccumulateUnused
        );
        usage_state
    } else {
        UsageState {
            remaining_in_period: payload.quantity_constraints.max_per_period,
            last_reset: modified_time_constraints.start,
        }
    };

    Ok(SpendingLimitPolicy {
        spending_limit: SpendingLimitV2 {
            mint: payload.mint,
            time_constraints: modified_time_constraints,
            quantity_constraints: payload.quantity_constraints,
            usage: usage_state,
        },
        source_account_index: payload.source_account_index,
        destinations,
    })
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

/// Anchor-returning validate_payload dispatch helper.
pub trait SpendingLimitPolicyExt {
    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &SpendingLimitPayload,
    ) -> Result<()>;
}

impl SpendingLimitPolicyExt for SpendingLimitPolicy {
    fn validate_payload(
        &self,
        _context: PolicyExecutionContext,
        payload: &SpendingLimitPayload,
    ) -> Result<()> {
        if !self.destinations.is_empty() {
            require!(
                self.destinations.contains(&payload.destination),
                SmartAccountError::InvalidDestination
            );
        }
        Ok(())
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
        SpendingLimitPolicy::invariant(self).to_anchor()
    }

    /// Validate that the destination is allowed
    fn validate_payload(
        &self,
        // No difference between synchronous and asynchronous execution
        context: PolicyExecutionContext,
        payload: &Self::UsagePayload,
    ) -> Result<()> {
        SpendingLimitPolicyExt::validate_payload(self, context, payload)
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
        self.spending_limit.is_active(current_timestamp).to_anchor()?;

        // Reset the period & amount
        self.spending_limit.reset_if_needed(current_timestamp);

        // Check that the amount complies with the spending limit
        self.spending_limit.check_amount(payload.amount).to_anchor()?;

        // Validate the accounts
        let validated_accounts = validate_accounts(self, &args.settings_key, payload, accounts)?;

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
        self.invariant().to_anchor()?;
        Ok(())
    }
}

// =============================================================================
// ACCOUNT VALIDATION
// =============================================================================

fn validate_accounts<'info>(
    policy: &SpendingLimitPolicy,
    settings_key: &Pubkey,
    args: &SpendingLimitPayload,
    accounts: &'info [AccountInfo<'info>],
) -> Result<ValidatedAccounts<'info>> {
    // Derive source account key
    let source_account_index_bytes = policy.source_account_index.to_le_bytes();
    let source_account_seeds = get_smart_account_seeds(settings_key, &source_account_index_bytes);

    // Derive source and destination account keys
    let (source_account_key, source_account_bump) =
        Pubkey::find_program_address(source_account_seeds.as_slice(), &crate::ID);

    // Mint specific logic
    match policy.spending_limit.mint {
        // Native SOL transfer
        mint if mint == Pubkey::default() => {
            // Parse out the accounts
            let (source_account_info, destination_account_info, system_program) =
                if let [source_account_info, destination_account_info, system_program, _remaining @ ..] =
                    accounts
                {
                    (source_account_info, destination_account_info, system_program)
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
            // Check that the source account is not the same as the destination account
            require!(
                source_account_info.key() != destination_account_info.key(),
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

            // Check the source account key
            require!(
                source_account_key == source_account_info.key(),
                SmartAccountError::InvalidAccount
            );

            // Deserialize the source and destination token accounts. Either
            // T22 or TokenKeg accounts
            let source_token_account =
                InterfaceAccount::<'info, TokenAccount>::try_from(source_token_account_info)?;
            let destination_token_account =
                InterfaceAccount::<TokenAccount>::try_from(destination_token_account_info)?;

            // Check the mint against the policy state
            require_eq!(policy.spending_limit.mint, mint.key());

            // Assert the ownership and mint of the token accounts
            require!(
                source_token_account.owner == source_account_key
                    && source_token_account.mint == policy.spending_limit.mint,
                SmartAccountError::InvalidAccount
            );
            require!(
                destination_token_account.owner == args.destination
                    && destination_token_account.mint == policy.spending_limit.mint,
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
