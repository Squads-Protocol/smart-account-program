use crate::generated::accounts::SpendingLimit;
use crate::generated::errors::SquadsSmartAccountProgramError as Error;

pub trait SpendingLimitExt {
    /// Anchor account size for a `SpendingLimit` with the given signer and
    /// destination counts.
    fn size(signers_length: usize, destinations_length: usize) -> usize;
    fn invariant(&self) -> Result<(), Error>;
}

impl SpendingLimitExt for SpendingLimit {
    fn size(signers_length: usize, destinations_length: usize) -> usize {
        8   // discriminator
        + 32  // settings
        + 32  // seed
        + 1   // account_index
        + 32  // mint
        + 8   // amount
        + 1   // Period enum
        + 8   // remaining_amount
        + 8   // last_reset
        + 1   // bump
        + 4 + signers_length * 32       // signers
        + 4 + destinations_length * 32  // destinations
        + 8 // expiration
    }

    fn invariant(&self) -> Result<(), Error> {
        if self.amount == 0 {
            return Err(Error::SpendingLimitInvalidAmount);
        }
        if self.signers.is_empty() {
            return Err(Error::EmptySigners);
        }
        let has_duplicates = self.signers.windows(2).any(|w| w[0] == w[1]);
        if has_duplicates {
            return Err(Error::DuplicateSigner);
        }
        Ok(())
    }
}
