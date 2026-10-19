use solana_address::Address;

use crate::generated::accounts::ProgramConfig;
use crate::generated::errors::SquadsSmartAccountProgramError as Error;

pub trait ProgramConfigExt {
    /// Anchor account size of a `ProgramConfig` (fixed; the `_reserved` field
    /// is a `[u8; 64]` so there are no dynamic Vec fields).
    const INIT_SPACE: usize = 8  // discriminator
        + 16  // smart_account_index
        + 32  // authority
        + 8   // smart_account_creation_fee
        + 32  // treasury
        + 64; // _reserved

    fn invariant(&self) -> Result<(), Error>;
    fn increment_smart_account_index(&mut self) -> Result<(), Error>;
}

impl ProgramConfigExt for ProgramConfig {
    fn invariant(&self) -> Result<(), Error> {
        let default_addr = Address::new_from_array([0u8; 32]);
        if self.authority == default_addr {
            return Err(Error::InvalidAccount);
        }
        if self.treasury == default_addr {
            return Err(Error::InvalidAccount);
        }
        Ok(())
    }

    fn increment_smart_account_index(&mut self) -> Result<(), Error> {
        self.smart_account_index = self
            .smart_account_index
            .checked_add(1)
            .ok_or(Error::InvalidAccount)?;
        Ok(())
    }
}
