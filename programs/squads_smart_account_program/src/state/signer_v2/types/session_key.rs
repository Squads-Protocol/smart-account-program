use anchor_lang::prelude::*;
use super::super::SESSION_KEY_EXPIRATION_LIMIT;

// ============================================================================
// Session Key Management (Shared Implementation)
// ============================================================================

/// Session key data shared by all external signer types.
/// Extracted into a separate struct to eliminate code duplication.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct SessionKeyData {
    /// Optional session key pubkey. Pubkey::default() means no session key.
    pub key: Pubkey,
    /// Session key expiration timestamp (Unix seconds). 0 if no session key.
    pub expiration: u64,
}

impl SessionKeyData {
    pub const SIZE: usize = 32 + 8; // 40 bytes

    /// Check if session key is active (not default and not expired)
    #[inline]
    pub fn is_active(&self, current_timestamp: u64) -> bool {
        self.key != Pubkey::default() && self.expiration > current_timestamp
    }

    /// Check if a given pubkey matches this session key and is active
    #[inline]
    pub fn matches(&self, pubkey: &Pubkey, current_timestamp: u64) -> bool {
        self.key == *pubkey && self.is_active(current_timestamp)
    }

    /// Clear session key
    #[inline]
    pub fn clear(&mut self) {
        self.key = Pubkey::default();
        self.expiration = 0;
    }

    /// Set session key with validation
    pub fn set(&mut self, key: Pubkey, expiration: u64, current_timestamp: u64) -> Result<()> {
        // Session key must not be the default pubkey
        if key == Pubkey::default() {
            return Err(error!(crate::errors::SmartAccountError::InvalidSessionKey));
        }
        // Session key expiration must be strictly in the future
        // (is_active checks expiration > current_timestamp, so expiration == current_timestamp would be immediately invalid)
        if expiration <= current_timestamp {
            return Err(error!(crate::errors::SmartAccountError::InvalidSessionKeyExpiration));
        }
        // Session key expiration must not exceed the limit (3 months from now)
        if expiration > current_timestamp.saturating_add(SESSION_KEY_EXPIRATION_LIMIT) {
            return Err(error!(crate::errors::SmartAccountError::SessionKeyExpirationTooLong));
        }
        self.key = key;
        self.expiration = expiration;
        Ok(())
    }
}
