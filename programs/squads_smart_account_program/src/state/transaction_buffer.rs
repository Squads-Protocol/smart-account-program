use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash;

use crate::errors::SmartAccountError;

pub const MAX_BUFFER_SIZE: usize = 4000;

#[account]
#[derive(Default, Debug)]
pub struct TransactionBuffer {
    /// The settings this belongs to.
    pub settings: Pubkey,
    /// Signer of the smart account who created the TransactionBuffer.
    pub creator: Pubkey,
    /// Index to seed address derivation
    pub buffer_index: u8,
    /// Smart account index of the transaction this buffer belongs to.
    pub account_index: u8,
    /// Hash of the final assembled transaction message.
    pub final_buffer_hash: [u8; 32],
    /// The size of the final assembled transaction message.
    pub final_buffer_size: u16,
    /// The buffer of the transaction message.
    pub buffer: Vec<u8>,
}

impl TransactionBuffer {
    pub fn size(final_message_buffer_size: u16) -> Result<usize> {
        // Make sure final size is not greater than MAX_BUFFER_SIZE bytes.
        if (final_message_buffer_size as usize) > MAX_BUFFER_SIZE {
            return err!(SmartAccountError::FinalBufferSizeExceeded);
        }
        Ok(
            8 +   // anchor account discriminator
            32 +  // multisig
            32 +  // creator
            1 +   // buffer_index
            1 +   // vault_index
            32 +  // transaction_message_hash
            2 +  // final_buffer_size
            4 + // vec length bytes
            final_message_buffer_size as usize, // buffer
        )
    }

    pub fn validate_hash(&self) -> Result<()> {
        let message_buffer_hash = hash(&self.buffer);
        require!(
            message_buffer_hash.to_bytes() == self.final_buffer_hash,
            SmartAccountError::FinalBufferHashMismatch
        );
        Ok(())
    }
    pub fn validate_size(&self) -> Result<()> {
        require_eq!(
            self.buffer.len(),
            self.final_buffer_size as usize,
            SmartAccountError::FinalBufferSizeMismatch
        );
        Ok(())
    }

    pub fn invariant(&self) -> Result<()> {
        require!(
            self.final_buffer_size as usize <= MAX_BUFFER_SIZE,
            SmartAccountError::FinalBufferSizeExceeded
        );
       
        require!(
            self.buffer.len() <= self.final_buffer_size as usize,
            SmartAccountError::FinalBufferSizeMismatch
        );

        Ok(())
    }
}
