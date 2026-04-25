use solana_program::pubkey::Pubkey;

use crate::state::policies::policy_core::PolicySizeTrait;
use crate::state::policies::utils::{
    QuantityConstraints, SpendingLimitV2, TimeConstraints, UsageState,
};

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpendingLimitPolicy {
    pub source_account_index: u8,
    pub destinations: Vec<Pubkey>,
    pub spending_limit: SpendingLimitV2,
}

impl SpendingLimitPolicy {
    pub fn invariant(&self) -> Result<(), crate::errors::SmartAccountError> {
        let has_duplicates = self.destinations.windows(2).any(|w| w[0] == w[1]);
        if has_duplicates {
            return Err(
                crate::errors::SmartAccountError::SpendingLimitPolicyInvariantDuplicateDestinations,
            );
        }
        self.spending_limit.invariant()?;
        Ok(())
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpendingLimitPolicyCreationPayload {
    pub mint: Pubkey,
    pub source_account_index: u8,
    pub time_constraints: TimeConstraints,
    pub quantity_constraints: QuantityConstraints,
    pub usage_state: Option<UsageState>,
    pub destinations: Vec<Pubkey>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpendingLimitPayload {
    pub amount: u64,
    pub destination: Pubkey,
    pub decimals: u8,
}

pub struct SpendingLimitExecutionArgs {
    pub settings_key: Pubkey,
}

impl PolicySizeTrait for SpendingLimitPolicyCreationPayload {
    fn creation_payload_size(&self) -> usize {
        32 + 1
            + TimeConstraints::INIT_SPACE
            + QuantityConstraints::INIT_SPACE
            + 4
            + self.destinations.len() * 32
    }

    fn policy_state_size(&self) -> usize {
        32 + TimeConstraints::INIT_SPACE
            + QuantityConstraints::INIT_SPACE
            + UsageState::INIT_SPACE
            + 1
            + 4
            + self.destinations.len() * 32
    }
}
