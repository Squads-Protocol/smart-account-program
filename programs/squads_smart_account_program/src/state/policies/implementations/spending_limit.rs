use anchor_lang::{prelude::*, system_program, Ids};
use anchor_spl::token_interface::{self, TokenAccount, TokenInterface, TransferChecked};

use crate::{
    errors::*, get_smart_account_seeds, state::utils::{PeriodV2, QuantityConstraints, ResourceLimit, TimeConstraints, UsageState}, PolicyPayloadConversionTrait,
    PolicySizeTrait, PolicyTrait, SEED_PREFIX, SEED_SMART_ACCOUNT,
};

/// Enhanced period enum supporting custom durations

/// Main spending limit structure using composition
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct SpendingLimitPolicy {
    /// Resource limit configuration (timing, constraints, usage, mint)
    pub resource_limit: ResourceLimit,

    /// The source account index.
    pub source_account_index: u8,

    /// The destination addresses the spending limit is allowed to sent funds to.
    /// If empty, funds can be sent to any address.
    pub destinations: Vec<Pubkey>,
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
            resource_limit: ResourceLimit {
                mint: self.mint,
                time_constraints: self.time_constraints,
                quantity_constraints: self.quantity_constraints,
                usage: UsageState {
                    remaining_in_period: self.quantity_constraints.max_per_period,
                    last_reset: now,
                },
            },
            source_account_index: self.source_account_index,
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
        32 + // mint (in ResourceLimit)
        TimeConstraints::INIT_SPACE + // timing (in ResourceLimit)
        QuantityConstraints::INIT_SPACE + // constraints (in ResourceLimit)
        UsageState::INIT_SPACE + // usage (in ResourceLimit)
        1 + // source_account_index
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
        self.resource_limit.invariant()?;
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
        self.resource_limit.reset_if_needed(current_timestamp);

        // Check that the amount complies with the resource limit
        self.resource_limit.check_amount(payload.amount)?;

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
        self.resource_limit.decrement(payload.amount);

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
        match self.resource_limit.mint {
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
                require_eq!(self.resource_limit.mint, mint.key());

                // Assert the ownership and mint of the token accounts
                require!(
                    source_token_account.owner == source_account_key
                        && source_token_account.mint == self.resource_limit.mint,
                    SmartAccountError::InvalidAccount
                );
                require!(
                    destination_token_account.owner == args.destination
                        && destination_token_account.mint == self.resource_limit.mint,
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


