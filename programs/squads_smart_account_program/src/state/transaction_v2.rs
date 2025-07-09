use anchor_lang::prelude::*;
use anchor_lang::solana_program::borsh0_10::get_instance_packed_len;

use crate::errors::*;
use crate::instructions::{CompiledInstruction, MessageAddressTableLookup, TransactionMessage};

use super::SmartAccountTransactionMessage;

/// Stores data required for tracking the voting and execution status of a smart
/// account transaction or policy action
/// Smart Account transaction is a transaction that's executed on behalf of the
/// smart account PDA
/// and wraps arbitrary Solana instructions, typically calling into other Solana programs.
#[account]
#[derive(Default)]
pub struct Transaction {
    /// The consensus account this belongs to.
    pub consensus_account: Pubkey,
    /// Signer of the Smart Account who submitted the transaction.
    pub creator: Pubkey,
    /// The rent collector for the transaction account.
    pub rent_collector: Pubkey,
    /// Index of this transaction within the smart account.
    pub index: u64,
    /// bump for the transaction seeds.
    pub bump: u8,
    /// The account index of the smart account this transaction belongs to.
    pub account_index: u8,
    /// Derivation bump of the smart account PDA this transaction belongs to.
    pub account_bump: u8,
    pub transaction_payload: Option<TransactionPayloadDetails>,
    pub policy_action_payload: Option<PolicyActionPayloadDetails>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TransactionPayloadDetails {
    pub ephemeral_signer_bumps: Vec<u8>,
    pub message: SmartAccountTransactionMessage,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PolicyActionPayloadDetails {
    // pub message: PolicyAction,
}

impl Transaction {
    pub fn size(ephemeral_signers_length: u8, transaction_message: &[u8]) -> Result<usize> {
        let transaction_message: SmartAccountTransactionMessage =
            TransactionMessage::deserialize(&mut &transaction_message[..])?.try_into()?;
        let message_size = get_instance_packed_len(&transaction_message).unwrap_or_default();

        Ok(
            8 +   // anchor account discriminator
            32 +  // settings
            32 +  // creator
            32 +  // rent_collector
            8 +   // index
            1 +   // bump
            1 +   // account_index
            1 +   // account_bump
            (4 + usize::from(ephemeral_signers_length)) +   // ephemeral_signers_bumps vec
            message_size, // message
        )
    }
    /// Reduces the Transaction to its default empty value and moves
    /// ownership of the data to the caller/return value.
    pub fn take(&mut self) -> Transaction {
        core::mem::take(self)
    }
}
