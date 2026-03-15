use crate::errors::SmartAccountError;
use anchor_lang::prelude::*;

/// Minimum authenticator data length: 32 (rpIdHash) + 1 (flags) + 4 (counter) = 37 bytes
const AUTH_DATA_MIN_LEN: usize = 37;

/// Wrapper for parsing WebAuthn authenticator data
pub struct AuthDataParser<'a> {
    auth_data: &'a [u8],
}

impl<'a> AuthDataParser<'a> {
    /// Creates a new AuthDataParser with bounds validation
    pub fn new(auth_data: &'a [u8]) -> Result<Self> {
        require!(
            auth_data.len() >= AUTH_DATA_MIN_LEN,
            SmartAccountError::InvalidPrecompileData
        );
        Ok(Self { auth_data })
    }

    /// Gets the RP ID hash (first 32 bytes)
    pub fn rp_id_hash(&self) -> &'a [u8] {
        &self.auth_data[0..32]
    }

    /// Checks if the user is present based on the flags
    pub fn is_user_present(&self) -> bool {
        self.auth_data[32] & 0x01 != 0
    }

    /// Checks if the user is verified based on the flags
    pub fn is_user_verified(&self) -> bool {
        self.auth_data[32] & 0x04 != 0
    }

    /// Gets the counter from the authenticator data (bytes 33-36, big-endian)
    pub fn get_counter(&self) -> u32 {
        u32::from_be_bytes([
            self.auth_data[33],
            self.auth_data[34],
            self.auth_data[35],
            self.auth_data[36],
        ])
    }
}
