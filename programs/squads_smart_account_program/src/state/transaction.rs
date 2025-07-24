use anchor_lang::prelude::*;
use anchor_lang::solana_program::borsh0_10::get_instance_packed_len;

use crate::errors::*;
use crate::instructions::{CompiledInstruction, MessageAddressTableLookup, TransactionMessage};

use super::{PolicyPayload, SmartAccountTransactionMessage};

/// Stores data required for tracking the voting and execution status of a smart
/// account transaction or policy action
/// Smart Account transaction is a transaction that's executed on behalf of the
/// smart account PDA
/// and wraps arbitrary Solana instructions, typically calling into other Solana programs.
#[account]
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

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum Payload {
    TransactionPayload(TransactionPayloadDetails),
    PolicyPayload(PolicyActionPayloadDetails),
}

impl Payload {
    pub fn transaction_payload(&self) -> Result<&TransactionPayloadDetails> {
        match self {
            Payload::TransactionPayload(payload) => Ok(payload),
            _ => err!(SmartAccountError::InvalidPayload),
        }
    }

    pub fn policy_payload(&self) -> Result<&PolicyActionPayloadDetails> {
        match self {
            Payload::PolicyPayload(payload) => Ok(payload),
            _ => err!(SmartAccountError::InvalidPayload),
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Eq, PartialEq)]
pub struct TransactionPayloadDetails {
    /// The account index of the smart account this transaction belongs to.
    pub account_index: u8,
    /// The ephemeral signer bumps for the transaction.
    pub ephemeral_signer_bumps: Vec<u8>,
    /// The message of the transaction.
    pub message: SmartAccountTransactionMessage,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PolicyActionPayloadDetails {
    pub payload: PolicyPayload,
}

impl Transaction {
    pub fn size_for_transaction(ephemeral_signers_length: u8, transaction_message: &[u8]) -> Result<usize> {
        let transaction_message: SmartAccountTransactionMessage =
            TransactionMessage::deserialize(&mut &transaction_message[..])?.try_into()?;

        let payload = Payload::TransactionPayload(TransactionPayloadDetails {
            account_index: 0,
            ephemeral_signer_bumps: vec![0; usize::from(ephemeral_signers_length)],
            message: transaction_message,
        });

        let payload_size = get_instance_packed_len(&payload).unwrap_or_default();

        Ok(
            8 +   // anchor account discriminator
            32 +  // consensus_account
            32 +  // creator
            32 +  // rent_collector
            8 +   // index
            1 +  // account_index
            1 +  // account_bump
            payload_size, // payload
        )
    }

    pub fn size_for_policy(payload: &PolicyPayload) -> Result<usize> {
        let payload_enum = Payload::PolicyPayload(PolicyActionPayloadDetails {
            payload: payload.clone(),
        });

        let payload_size = get_instance_packed_len(&payload_enum).unwrap_or_default();

        Ok(
            8 +   // anchor account discriminator
            32 +  // consensus_account
            32 +  // creator
            32 +  // rent_collector
            8 +   // index
            1 +  // account_index
            1 +  // account_bump
            payload_size, // payload
        )
    }

    pub fn size(ephemeral_signers_length: u8, transaction_message: &[u8]) -> Result<usize> {
        // Backward compatibility - assume transaction payload
        Self::size_for_transaction(ephemeral_signers_length, transaction_message)
    }
}
