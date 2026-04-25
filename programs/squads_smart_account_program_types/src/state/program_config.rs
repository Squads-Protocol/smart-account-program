use solana_program::pubkey::Pubkey;

use crate::errors::SmartAccountError;

/// Global program configuration account.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub struct ProgramConfig {
    /// Counter for the number of smart accounts created.
    pub smart_account_index: u128,
    /// The authority which can update the config.
    pub authority: Pubkey,
    /// The lamports amount charged for creating a new smart account.
    pub smart_account_creation_fee: u64,
    /// The treasury account to send charged fees to.
    pub treasury: Pubkey,
    /// Reserved for future use.
    pub _reserved: [u8; 64],
}

impl ProgramConfig {
    pub const DISCRIMINATOR: [u8; 8] = [0xc4, 0xd2, 0x5a, 0xe7, 0x90, 0x95, 0x8c, 0x3f];

    /// Byte size consumed by the struct when serialized by anchor's `InitSpace` derive.
    pub const INIT_SPACE: usize = 16 + 32 + 8 + 32 + 64;

    pub fn invariant(&self) -> Result<(), SmartAccountError> {
        if self.authority == Pubkey::default() {
            return Err(SmartAccountError::InvalidAccount);
        }
        if self.treasury == Pubkey::default() {
            return Err(SmartAccountError::InvalidAccount);
        }
        Ok(())
    }

    pub fn increment_smart_account_index(&mut self) -> Result<(), SmartAccountError> {
        self.smart_account_index = self.smart_account_index.checked_add(1).unwrap();
        Ok(())
    }
}

#[cfg(feature = "borsh")]
impl ProgramConfig {
    pub fn try_deserialize(data: &[u8]) -> Result<Self, borsh::maybestd::io::Error> {
        if data.len() < 8 {
            return Err(borsh::maybestd::io::Error::new(
                borsh::maybestd::io::ErrorKind::InvalidData,
                "account data shorter than discriminator",
            ));
        }
        if data[..8] != Self::DISCRIMINATOR {
            return Err(borsh::maybestd::io::Error::new(
                borsh::maybestd::io::ErrorKind::InvalidData,
                "discriminator mismatch",
            ));
        }
        let mut body = &data[8..];
        <Self as borsh::BorshDeserialize>::deserialize(&mut body)
    }
}
