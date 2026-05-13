use solana_pubkey::Pubkey;

use crate::errors::SmartAccountError;

pub const MAX_BUFFER_SIZE: usize = 4000;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Default, Debug)]
pub struct TransactionBuffer {
    /// The consensus account (settings or policy) this belongs to.
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
    pub const DISCRIMINATOR: [u8; 8] = [0x5a, 0x24, 0x23, 0xdb, 0x5d, 0xe1, 0x6e, 0x60];

    pub fn size(final_message_buffer_size: u16) -> Result<usize, SmartAccountError> {
        if (final_message_buffer_size as usize) > MAX_BUFFER_SIZE {
            return Err(SmartAccountError::FinalBufferSizeExceeded);
        }
        Ok(8 +   // anchor account discriminator
            32 +  // settings
            32 +  // creator
            1 +   // buffer_index
            1 +   // account_index
            32 +  // final_buffer_hash
            2 +   // final_buffer_size
            4 +   // buffer vec length
            final_message_buffer_size as usize)
    }

    pub fn invariant(&self) -> Result<(), SmartAccountError> {
        if self.final_buffer_size as usize > MAX_BUFFER_SIZE {
            return Err(SmartAccountError::FinalBufferSizeExceeded);
        }
        if self.buffer.len() > self.final_buffer_size as usize {
            return Err(SmartAccountError::FinalBufferSizeMismatch);
        }
        Ok(())
    }

    /// Validate that `self.buffer` is exactly `self.final_buffer_size` bytes.
    pub fn validate_size(&self) -> Result<(), SmartAccountError> {
        if self.buffer.len() != self.final_buffer_size as usize {
            return Err(SmartAccountError::FinalBufferSizeMismatch);
        }
        Ok(())
    }
}

#[cfg(feature = "borsh")]
impl TransactionBuffer {
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
}
