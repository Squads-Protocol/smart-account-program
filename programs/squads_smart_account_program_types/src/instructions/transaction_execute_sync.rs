use crate::errors::SmartAccountError;
use crate::state::PolicyPayload;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub enum SyncPayload {
    Transaction(Vec<u8>),
    Policy(PolicyPayload),
}

impl SyncPayload {
    pub fn to_transaction_payload(&self) -> Result<&Vec<u8>, SmartAccountError> {
        match self {
            SyncPayload::Transaction(payload) => Ok(payload),
            _ => Err(SmartAccountError::InvalidPayload),
        }
    }

    pub fn to_policy_payload(&self) -> Result<&PolicyPayload, SmartAccountError> {
        match self {
            SyncPayload::Policy(payload) => Ok(payload),
            _ => Err(SmartAccountError::InvalidPayload),
        }
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct SyncTransactionArgs {
    pub account_index: u8,
    pub num_signers: u8,
    pub payload: SyncPayload,
}
