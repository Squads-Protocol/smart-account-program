use crate::generated::accounts::{Batch, BatchTransaction};
use crate::generated::errors::SquadsSmartAccountProgramError as Error;

pub trait BatchExt {
    /// Bytes consumed by a `Batch` on-chain (fixed; no dynamic Vec fields).
    /// Includes the 8-byte discriminator.
    const INIT_SPACE: usize = 8  // discriminator
        + 32  // settings
        + 32  // creator
        + 32  // rent_collector
        + 8   // index
        + 1   // bump
        + 1   // account_index
        + 1   // account_bump
        + 4   // size
        + 4; // executed_transaction_index

    fn invariant(&self) -> Result<(), Error>;
}

impl BatchExt for Batch {
    fn invariant(&self) -> Result<(), Error> {
        if self.size < self.executed_transaction_index {
            return Err(Error::InvalidTransactionIndex);
        }
        Ok(())
    }
}

pub trait BatchTransactionExt {
    /// Reset the BatchTransaction to its default and return the original.
    fn take(&mut self) -> Self;
}

impl BatchTransactionExt for BatchTransaction {
    fn take(&mut self) -> Self {
        // BatchTransaction doesn't derive Default (codama doesn't emit it), so
        // we replace fields manually.
        let mut taken = self.clone();
        std::mem::swap(&mut taken, self);
        // Best-effort "default" — leave a small placeholder. Callers shouldn't
        // rely on the after-state beyond "it's safe to drop".
        self.bump = 0;
        self.rent_collector = solana_address::Address::new_from_array([0u8; 32]);
        self.ephemeral_signer_bumps.clear();
        self.message.account_keys.clear();
        self.message.instructions.clear();
        self.message.address_table_lookups.clear();
        self.message.num_signers = 0;
        self.message.num_writable_signers = 0;
        self.message.num_writable_non_signers = 0;
        taken
    }
}
