use crate::{
    errors::SmartAccountError, get_smart_account_seeds, state::policies::policy_core::PolicyTrait,
    SEED_PREFIX, SEED_SMART_ACCOUNT,
};
use crate::{PolicyExecutionContext, PolicyPayloadConversionTrait, PolicySizeTrait};
use anchor_lang::prelude::InterfaceAccount;
use anchor_lang::{prelude::*, system_program, Ids};
use anchor_spl::token_interface::{self, TokenAccount, TokenInterface, TransferChecked};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct InternalFundTransferPolicy {
    // Using bitmasks here not only saves space, but it also prevents us from
    // having to deduplicate or sort the account indices.
    // Bitmask of allowed source account indices
    pub source_account_mask: [u8; 32],
    // Bitmask of allowed destination account indices
    pub destination_account_mask: [u8; 32],
    pub allowed_mints: Vec<Pubkey>,
}

impl InternalFundTransferPolicy {
    pub fn mask_to_indices(mask: &[u8; 32]) -> Vec<u8> {
        let mut indices = Vec::new();
        for i in 0..32 {
            for j in 0..8 {
                if mask[i] & (1 << j) != 0 {
                    indices.push((i * 8 + j) as u8);
                }
            }
        }
        indices
    }

    pub fn indices_to_mask(indices: &[u8]) -> [u8; 32] {
        let mut mask = [0u8; 32];
        for index in indices {
            mask[*index as usize / 8] |= 1 << (*index as usize % 8);
        }
        mask
    }
    /// Checks if the given index is in the given mask
    pub fn has_account_index(&self, index: u8, mask: &[u8; 32]) -> bool {
        let byte_idx = (index / 8) as usize;
        let bit_idx = index % 8;
        (mask[byte_idx] & (1 << bit_idx)) != 0
    }

    /// Checks if the given index is in the source account mask
    pub fn has_source_account_index(&self, index: u8) -> bool {
        self.has_account_index(index, &self.source_account_mask)
    }

    /// Checks if the given index is in the destination account mask
    pub fn has_destination_account_index(&self, index: u8) -> bool {
        self.has_account_index(index, &self.destination_account_mask)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct InternalFundTransferPayload {
    pub source_index: u8,
    pub destination_index: u8,
    pub mint: Pubkey,
    pub decimals: u8,
    pub amount: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct InternalFundTransferPolicyCreationPayload {
    pub source_account_indices: Vec<u8>,
    pub destination_account_indices: Vec<u8>,
    pub allowed_mints: Vec<Pubkey>,
}

impl PolicySizeTrait for InternalFundTransferPolicyCreationPayload {
    fn creation_payload_size(&self) -> usize {
        4 + self.source_account_indices.len() + // source_account_indices vec
        4 + self.destination_account_indices.len() + // destination_account_indices vec
        4 + self.allowed_mints.len() * 32 // allowed_mints vec
    }

    fn policy_state_size(&self) -> usize {
        32 + // source_account_mask
        32 + // destination_account_mask
        4 + self.allowed_mints.len() * 32 // allowed_mints vec
    }
}

impl PolicyPayloadConversionTrait for InternalFundTransferPolicyCreationPayload {
    type PolicyState = InternalFundTransferPolicy;

    fn to_policy_state(self) -> Result<InternalFundTransferPolicy> {
        // Sort the allowed mints to ensure the invariant function can apply.
        let mut sorted_allowed_mints = self.allowed_mints.clone();
        sorted_allowed_mints.sort_by_key(|mint| mint.clone());

        // Create the policy state
        Ok(InternalFundTransferPolicy {
            source_account_mask: InternalFundTransferPolicy::indices_to_mask(
                &self.source_account_indices,
            ),
            destination_account_mask: InternalFundTransferPolicy::indices_to_mask(
                &self.destination_account_indices,
            ),
            allowed_mints: sorted_allowed_mints,
        })
    }
}

pub struct InternalFundTransferExecutionArgs {
    pub settings_key: Pubkey,
}

impl PolicyTrait for InternalFundTransferPolicy {
    type PolicyState = Self;
    type CreationPayload = InternalFundTransferPolicyCreationPayload;
    type UsagePayload = InternalFundTransferPayload;
    type ExecutionArgs = InternalFundTransferExecutionArgs;

    fn invariant(&self) -> Result<()> {
        // There can't be duplicate mints. Requires the mints are sorted.
        let has_duplicates = self.allowed_mints.windows(2).any(|win| win[0] == win[1]);
        require!(
            !has_duplicates,
            SmartAccountError::InternalFundTransferPolicyInvariantDuplicateMints
        );
        Ok(())
    }

    /// Validates a given usage payload.
    fn validate_payload(
        &self,
        // No difference between synchronous and asynchronous execution
        _context: PolicyExecutionContext,
        payload: &Self::UsagePayload,
    ) -> Result<()> {
        // Validate source account index is allowed
        require!(
            self.has_source_account_index(payload.source_index),
            SmartAccountError::InternalFundTransferPolicyInvariantSourceAccountIndexNotAllowed
        );

        // Validate destination account index is allowed
        require!(
            self.has_destination_account_index(payload.destination_index),
            SmartAccountError::InternalFundTransferPolicyInvariantDestinationAccountIndexNotAllowed
        );

        // Validate mint is allowed (empty allowed_mints means all mints are allowed)
        if !self.allowed_mints.is_empty() {
            require!(
                self.allowed_mints.contains(&payload.mint),
                SmartAccountError::InternalFundTransferPolicyInvariantMintNotAllowed
            );
        }

        // Validate amount is non-zero
        require!(
            payload.amount > 0,
            SmartAccountError::InternalFundTransferPolicyInvariantAmountZero
        );

        // Validate source and destination are different
        require!(
            payload.source_index != payload.destination_index,
            SmartAccountError::InternalFundTransferPolicyInvariantSourceAndDestinationCannotBeTheSame
        );

        Ok(())
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
        let validated_accounts = Self::validate_accounts(&args.settings_key, &payload, accounts)?;

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
impl InternalFundTransferPolicy {
    /// Validates the accounts passed in and returns a struct with the accounts
    fn validate_accounts<'info>(
        settings_key: &Pubkey,
        args: &InternalFundTransferPayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<ValidatedAccounts<'info>> {
        // Derive source and destination account keys
        let source_account_index_bytes = args.source_index.to_le_bytes();
        let destination_account_index_bytes = args.destination_index.to_le_bytes();
        let source_account_seeds =
            get_smart_account_seeds(settings_key, &source_account_index_bytes);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indices_to_mask_and_back() {
        let indices = vec![0, 1, 8, 15, 31, 63, 127, 255];
        let mask = InternalFundTransferPolicy::indices_to_mask(&indices);
        let result_indices = InternalFundTransferPolicy::mask_to_indices(&mask);
        assert_eq!(indices, result_indices);
    }

    #[test]
    fn test_has_account_index() {
        let indices = vec![2, 5, 10, 20];
        let mask = InternalFundTransferPolicy::indices_to_mask(&indices);
        let policy = InternalFundTransferPolicy {
            source_account_mask: mask,
            destination_account_mask: [0u8; 32],
            allowed_mints: vec![],
        };
        for &idx in &indices {
            assert!(policy.has_account_index(idx, &policy.source_account_mask));
        }
        assert!(!policy.has_account_index(3, &policy.source_account_mask));
        assert!(!policy.has_account_index(0, &policy.destination_account_mask));
    }

    #[test]
    fn test_has_source_and_destination_account_index() {
        let source_indices = vec![1, 3, 5];
        let dest_indices = vec![2, 4, 6];
        let policy = InternalFundTransferPolicy {
            source_account_mask: InternalFundTransferPolicy::indices_to_mask(&source_indices),
            destination_account_mask: InternalFundTransferPolicy::indices_to_mask(&dest_indices),
            allowed_mints: vec![],
        };
        for &idx in &source_indices {
            assert!(policy.has_source_account_index(idx));
        }
        for &idx in &dest_indices {
            assert!(policy.has_destination_account_index(idx));
        }
        assert!(!policy.has_source_account_index(2));
        assert!(!policy.has_destination_account_index(1));
    }
}
