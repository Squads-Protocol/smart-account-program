//! TransactionBuffer — re-exported from the types crate plus an extension
//! trait for methods that need `solana_program::hash::hash`.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash;

pub use squads_smart_account_program_types::{TransactionBuffer, MAX_BUFFER_SIZE};

use crate::error_conv::ToAnchorResult;
use crate::errors::SmartAccountError;

/// Program-side extensions for `TransactionBuffer`. The hashing method is
/// kept here because the types crate must not depend on solana_program::hash.
pub trait TransactionBufferExt {
    fn validate_hash(&self) -> Result<()>;
    fn validate_size(&self) -> Result<()>;
    fn invariant(&self) -> Result<()>;
}

impl TransactionBufferExt for TransactionBuffer {
    fn validate_hash(&self) -> Result<()> {
        let message_buffer_hash = hash(&self.buffer);
        require!(
            message_buffer_hash.to_bytes() == self.final_buffer_hash,
            SmartAccountError::FinalBufferHashMismatch
        );
        Ok(())
    }

    fn validate_size(&self) -> Result<()> {
        TransactionBuffer::validate_size(self).to_anchor()
    }

    fn invariant(&self) -> Result<()> {
        self.invariant().to_anchor()
    }
}
