use solana_program::pubkey::Pubkey;

use crate::errors::SmartAccountError;
use crate::state::policies::policy_core::{
    PolicyExecutionContext, PolicyPayloadConversionTrait, PolicySizeTrait,
};

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InternalFundTransferPolicy {
    /// Bitmask of allowed source account indices.
    pub source_account_mask: [u8; 32],
    /// Bitmask of allowed destination account indices.
    pub destination_account_mask: [u8; 32],
    pub allowed_mints: Vec<Pubkey>,
}

impl InternalFundTransferPolicy {
    /// Convert a bitmask to a list of indices.
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

    /// Convert a list of indices to a bitmask.
    pub fn indices_to_mask(indices: &[u8]) -> [u8; 32] {
        let mut mask = [0u8; 32];
        for index in indices {
            mask[*index as usize / 8] |= 1 << (*index as usize % 8);
        }
        mask
    }

    pub fn has_account_index(index: u8, mask: &[u8; 32]) -> bool {
        let byte_idx = (index / 8) as usize;
        let bit_idx = index % 8;
        (mask[byte_idx] & (1 << bit_idx)) != 0
    }

    pub fn has_source_account_index(&self, index: u8) -> bool {
        Self::has_account_index(index, &self.source_account_mask)
    }

    pub fn has_destination_account_index(&self, index: u8) -> bool {
        Self::has_account_index(index, &self.destination_account_mask)
    }

    pub fn invariant(&self) -> Result<(), SmartAccountError> {
        let has_duplicates = self.allowed_mints.windows(2).any(|win| win[0] == win[1]);
        if has_duplicates {
            return Err(SmartAccountError::InternalFundTransferPolicyInvariantDuplicateMints);
        }
        Ok(())
    }

    pub fn validate_payload(
        &self,
        _context: PolicyExecutionContext,
        payload: &InternalFundTransferPayload,
    ) -> Result<(), SmartAccountError> {
        if !self.has_source_account_index(payload.source_index) {
            return Err(
                SmartAccountError::InternalFundTransferPolicyInvariantSourceAccountIndexNotAllowed,
            );
        }

        if !self.has_destination_account_index(payload.destination_index) {
            return Err(
                SmartAccountError::InternalFundTransferPolicyInvariantDestinationAccountIndexNotAllowed,
            );
        }

        if !self.allowed_mints.is_empty() && !self.allowed_mints.contains(&payload.mint) {
            return Err(SmartAccountError::InternalFundTransferPolicyInvariantMintNotAllowed);
        }

        if payload.amount == 0 {
            return Err(SmartAccountError::InternalFundTransferPolicyInvariantAmountZero);
        }

        if payload.source_index == payload.destination_index {
            return Err(
                SmartAccountError::InternalFundTransferPolicyInvariantSourceAndDestinationCannotBeTheSame,
            );
        }

        Ok(())
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq)]
pub struct InternalFundTransferPayload {
    pub source_index: u8,
    pub destination_index: u8,
    pub mint: Pubkey,
    pub decimals: u8,
    pub amount: u64,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq)]
pub struct InternalFundTransferPolicyCreationPayload {
    pub source_account_indices: Vec<u8>,
    pub destination_account_indices: Vec<u8>,
    pub allowed_mints: Vec<Pubkey>,
}

impl PolicySizeTrait for InternalFundTransferPolicyCreationPayload {
    fn creation_payload_size(&self) -> usize {
        4 + self.source_account_indices.len()
            + 4 + self.destination_account_indices.len()
            + 4 + self.allowed_mints.len() * 32
    }

    fn policy_state_size(&self) -> usize {
        32 + 32 + 4 + self.allowed_mints.len() * 32
    }
}

impl PolicyPayloadConversionTrait for InternalFundTransferPolicyCreationPayload {
    type PolicyState = InternalFundTransferPolicy;

    fn to_policy_state(self) -> Result<InternalFundTransferPolicy, SmartAccountError> {
        let mut sorted_allowed_mints = self.allowed_mints.clone();
        sorted_allowed_mints.sort_by_key(|mint| *mint);

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
            assert!(InternalFundTransferPolicy::has_account_index(
                idx,
                &policy.source_account_mask
            ));
        }
        assert!(!InternalFundTransferPolicy::has_account_index(
            3,
            &policy.source_account_mask
        ));
        assert!(!InternalFundTransferPolicy::has_account_index(
            0,
            &policy.destination_account_mask
        ));
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
