use solana_program::pubkey::Pubkey;

use crate::errors::SmartAccountError;
use crate::state::{PolicyPayload, SmartAccountTransactionMessage};

/// Stores data required for tracking the voting and execution status of a smart
/// account transaction or policy action.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub struct Transaction {
    /// The consensus account this belongs to.
    pub consensus_account: Pubkey,
    /// Signer of the Smart Account who submitted the transaction.
    pub creator: Pubkey,
    /// The rent collector for the transaction account.
    pub rent_collector: Pubkey,
    /// Index of this transaction within the consensus account.
    pub index: u64,
    /// The payload of the transaction.
    pub payload: Payload,
}

impl Transaction {
    pub const DISCRIMINATOR: [u8; 8] = [0x0b, 0x18, 0xae, 0x81, 0xcb, 0x75, 0xf2, 0x17];
}

#[cfg(feature = "borsh")]
impl Transaction {
    pub fn try_deserialize(data: &[u8]) -> Result<Self, borsh::maybestd::io::Error> {
        if data.len() < 8 || data[..8] != Self::DISCRIMINATOR {
            return Err(borsh::maybestd::io::Error::new(
                borsh::maybestd::io::ErrorKind::InvalidData,
                "discriminator mismatch",
            ));
        }
        let mut body = &data[8..];
        <Self as borsh::BorshDeserialize>::deserialize(&mut body)
    }

    pub fn size_for_transaction(
        ephemeral_signers_length: u8,
        transaction_message: &[u8],
    ) -> Result<usize, SmartAccountError> {
        use crate::instructions::TransactionMessage;
        let transaction_message: SmartAccountTransactionMessage =
            <TransactionMessage as borsh::BorshDeserialize>::deserialize(
                &mut &transaction_message[..],
            )
            .map_err(|_| SmartAccountError::InvalidTransactionMessage)?
            .try_into()?;

        let payload = Payload::TransactionPayload(TransactionPayloadDetails {
            account_index: 0,
            ephemeral_signer_bumps: vec![0; usize::from(ephemeral_signers_length)],
            message: transaction_message,
        });

        let payload_size = borsh::to_vec(&payload).map(|v| v.len()).unwrap_or_default();

        Ok(8 +   // anchor account discriminator
            32 +  // consensus_account
            32 +  // creator
            32 +  // rent_collector
            8 +   // index
            1 +   // account_index
            1 +   // account_bump
            payload_size)
    }

    pub fn size_for_policy(payload: &PolicyPayload) -> Result<usize, SmartAccountError> {
        let payload_enum = Payload::PolicyPayload(PolicyActionPayloadDetails {
            payload: payload.clone(),
        });

        let payload_size = borsh::to_vec(&payload_enum)
            .map(|v| v.len())
            .unwrap_or_default();

        Ok(8 +   // anchor account discriminator
            32 +  // consensus_account
            32 +  // creator
            32 +  // rent_collector
            8 +   // index
            1 +   // account_index
            1 +   // account_bump
            payload_size)
    }

    pub fn size(
        ephemeral_signers_length: u8,
        transaction_message: &[u8],
    ) -> Result<usize, SmartAccountError> {
        Self::size_for_transaction(ephemeral_signers_length, transaction_message)
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub enum Payload {
    TransactionPayload(TransactionPayloadDetails),
    PolicyPayload(PolicyActionPayloadDetails),
}

impl Payload {
    pub fn transaction_payload(&self) -> Result<&TransactionPayloadDetails, SmartAccountError> {
        match self {
            Payload::TransactionPayload(payload) => Ok(payload),
            _ => Err(SmartAccountError::InvalidPayload),
        }
    }

    pub fn policy_payload(&self) -> Result<&PolicyActionPayloadDetails, SmartAccountError> {
        match self {
            Payload::PolicyPayload(payload) => Ok(payload),
            _ => Err(SmartAccountError::InvalidPayload),
        }
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Eq, PartialEq)]
pub struct TransactionPayloadDetails {
    pub account_index: u8,
    pub ephemeral_signer_bumps: Vec<u8>,
    pub message: SmartAccountTransactionMessage,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub struct PolicyActionPayloadDetails {
    pub payload: PolicyPayload,
}
