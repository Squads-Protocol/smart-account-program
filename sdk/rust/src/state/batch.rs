use solana_address::Address;

use crate::errors::SmartAccountError;
use crate::state::SmartAccountTransactionMessage;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub struct Batch {
    /// The consensus account (settings or policy) this belongs to.
    pub settings: Address,
    /// Signer of the smart account who submitted the batch.
    pub creator: Address,
    /// The rent collector for the batch account.
    pub rent_collector: Address,
    /// Index of this batch within the smart account transactions.
    pub index: u64,
    /// PDA bump.
    pub bump: u8,
    /// Index of the smart account this batch belongs to.
    pub account_index: u8,
    /// Derivation bump of the smart account PDA this batch belongs to.
    pub account_bump: u8,
    /// Number of transactions in the batch.
    pub size: u32,
    /// Index of the last executed transaction within the batch.
    pub executed_transaction_index: u32,
}

impl Batch {
    pub const DISCRIMINATOR: [u8; 8] = [0x9c, 0xc2, 0x46, 0x2c, 0x16, 0x58, 0x89, 0x2c];
    pub const INIT_SPACE: usize = 32 + 32 + 32 + 8 + 1 + 1 + 1 + 4 + 4;

    pub fn invariant(&self) -> Result<(), SmartAccountError> {
        if self.size < self.executed_transaction_index {
            return Err(SmartAccountError::InvalidTransactionIndex);
        }
        Ok(())
    }
}

#[cfg(feature = "borsh")]
impl Batch {
    pub fn try_deserialize(data: &[u8]) -> Result<Self, borsh::io::Error> {
        if data.len() < 8 || data[..8] != Self::DISCRIMINATOR {
            return Err(borsh::io::Error::new(
                borsh::io::ErrorKind::InvalidData,
                "discriminator mismatch",
            ));
        }
        let mut body = &data[8..];
        <Self as borsh::BorshDeserialize>::deserialize(&mut body)
    }
}

/// Stores data required for execution of one transaction from a batch.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Default)]
pub struct BatchTransaction {
    /// PDA bump.
    pub bump: u8,
    /// The rent collector for the batch transaction account.
    pub rent_collector: Address,
    /// Derivation bumps for additional signers.
    pub ephemeral_signer_bumps: Vec<u8>,
    /// data required for executing the transaction.
    pub message: SmartAccountTransactionMessage,
}

impl BatchTransaction {
    pub const DISCRIMINATOR: [u8; 8] = [0x5c, 0x14, 0x3d, 0x92, 0x9b, 0x3e, 0x70, 0x48];

    /// Reduces the BatchTransaction to its default empty value and moves
    /// ownership of the data to the caller/return value.
    pub fn take(&mut self) -> BatchTransaction {
        core::mem::take(self)
    }
}

#[cfg(feature = "borsh")]
impl BatchTransaction {
    pub fn try_deserialize(data: &[u8]) -> Result<Self, borsh::io::Error> {
        if data.len() < 8 || data[..8] != Self::DISCRIMINATOR {
            return Err(borsh::io::Error::new(
                borsh::io::ErrorKind::InvalidData,
                "discriminator mismatch",
            ));
        }
        let mut body = &data[8..];
        <Self as borsh::BorshDeserialize>::deserialize(&mut body)
    }

    /// Compute the serialized size of a `BatchTransaction` account with the
    /// given ephemeral-signer count and serialized `TransactionMessage` body.
    pub fn size(
        ephemeral_signers_length: u8,
        transaction_message: &[u8],
    ) -> Result<usize, borsh::io::Error> {
        use crate::instructions::TransactionMessage;
        let tm =
            <TransactionMessage as borsh::BorshDeserialize>::try_from_slice(transaction_message)?;
        let sm: SmartAccountTransactionMessage = tm.try_into().map_err(|_| {
            borsh::io::Error::new(
                borsh::io::ErrorKind::InvalidData,
                "invalid transaction message",
            )
        })?;
        let message_size = borsh::to_vec(&sm)?.len();
        Ok(8 +   // anchor account discriminator
            1 +   // bump
            32 +  // rent_collector
            (4 + usize::from(ephemeral_signers_length)) +   // ephemeral_signers_bumps vec
            message_size)
    }
}
