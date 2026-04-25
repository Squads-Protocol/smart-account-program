//! `InternalFundTransferPolicy` — payload types and pure validation live in
//! the types crate. The program-local pieces are:
//!   * `PolicyTrait` impl (local trait, foreign type — OK)
//!   * account validation + CPI execution (`ValidatedAccounts`)
//!   * `InternalFundTransferPolicyExt::validate_payload` so dispatch from
//!     `Policy::validate_payload` can return anchor-flavored `Result`

use anchor_lang::prelude::InterfaceAccount;
use anchor_lang::{prelude::*, system_program, Ids};
use anchor_spl::token_interface::{self, TokenAccount, TokenInterface, TransferChecked};

pub use squads_smart_account_program_types::{
    InternalFundTransferExecutionArgs, InternalFundTransferPayload, InternalFundTransferPolicy,
    InternalFundTransferPolicyCreationPayload,
};

use crate::error_conv::ToAnchorResult;
use crate::{
    errors::SmartAccountError,
    get_smart_account_seeds,
    state::policies::policy_core::{PolicyExecutionContext, PolicyTrait},
    SEED_PREFIX, SEED_SMART_ACCOUNT,
};

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

/// Anchor-returning validate_payload dispatch so the policy-level dispatch in
/// `PolicyExt::validate_payload` returns `anchor_lang::Result`.
pub trait InternalFundTransferPolicyExt {
    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &InternalFundTransferPayload,
    ) -> Result<()>;
}

impl InternalFundTransferPolicyExt for InternalFundTransferPolicy {
    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &InternalFundTransferPayload,
    ) -> Result<()> {
        InternalFundTransferPolicy::validate_payload(self, context, payload).to_anchor()
    }
}

impl PolicyTrait for InternalFundTransferPolicy {
    type PolicyState = Self;
    type CreationPayload = InternalFundTransferPolicyCreationPayload;
    type UsagePayload = InternalFundTransferPayload;
    type ExecutionArgs = InternalFundTransferExecutionArgs;

    fn invariant(&self) -> Result<()> {
        InternalFundTransferPolicy::invariant(self).to_anchor()
    }

    /// Validates a given usage payload.
    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &Self::UsagePayload,
    ) -> Result<()> {
        InternalFundTransferPolicy::validate_payload(self, context, payload).to_anchor()
    }

    /// Execute the internal fund transfer policy
    /// Expects the following accounts:
    /// - Source account
    /// - Source account token account
    /// - Destination account token account
    fn execute_payload<'info>(
        &mut self,
        args: Self::ExecutionArgs,
        payload: &Self::UsagePayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        let validated_accounts = validate_accounts(&args.settings_key, payload, accounts)?;

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
                            &payload.source_index.to_le_bytes(),
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
                            &payload.source_index.to_le_bytes(),
                            &[source_account_bump],
                        ]],
                    ),
                    payload.amount,
                    payload.decimals,
                )?;
            }
        }

        Ok(())
    }
}

/// Validates the accounts passed in and returns a struct with the accounts
fn validate_accounts<'info>(
    settings_key: &Pubkey,
    args: &InternalFundTransferPayload,
    accounts: &'info [AccountInfo<'info>],
) -> Result<ValidatedAccounts<'info>> {
    // Derive source and destination account keys
    let source_account_index_bytes = args.source_index.to_le_bytes();
    let destination_account_index_bytes = args.destination_index.to_le_bytes();
    let source_account_seeds = get_smart_account_seeds(settings_key, &source_account_index_bytes);
    let destination_account_seeds =
        get_smart_account_seeds(settings_key, &destination_account_index_bytes);

    // Derive source and destination account keys
    let (source_account_key, source_account_bump) =
        Pubkey::find_program_address(source_account_seeds.as_slice(), &crate::ID);
    // Derive the destination account from the destination index
    let (destination_account_key, _) =
        Pubkey::find_program_address(destination_account_seeds.as_slice(), &crate::ID);

    // Mint specific logic
    match args.mint {
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
                destination_account_key == destination_account_info.key(),
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
            // Check the mint against the payload
            require_eq!(args.mint, mint.key());

            // Assert the ownership and mint of the token accounts
            require!(
                source_token_account.owner == source_account_key
                    && source_token_account.mint == args.mint,
                SmartAccountError::InvalidAccount
            );
            require!(
                destination_token_account.owner == destination_account_key
                    && destination_token_account.mint == args.mint,
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
