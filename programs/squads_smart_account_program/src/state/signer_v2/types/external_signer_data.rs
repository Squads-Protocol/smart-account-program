use anchor_lang::prelude::*;
use super::SessionKeyData;

/// Trait for external signer data types that share session key management.
///
/// Implemented by P256WebauthnData, Secp256k1Data, and Ed25519ExternalData.
/// Provides default implementations for session key operations, eliminating
/// duplicated method bodies across the three types.
pub trait ExternalSignerData {
    fn session_key_data(&self) -> &SessionKeyData;
    fn session_key_data_mut(&mut self) -> &mut SessionKeyData;

    /// Check if session key is active (not default and not expired)
    #[inline]
    fn has_active_session_key(&self, current_timestamp: u64) -> bool {
        self.session_key_data().is_active(current_timestamp)
    }

    /// Clear session key
    #[inline]
    fn clear_session_key(&mut self) {
        self.session_key_data_mut().clear();
    }

    /// Set session key with validation
    fn set_session_key(&mut self, key: Pubkey, expiration: u64, current_timestamp: u64) -> Result<()> {
        self.session_key_data_mut().set(key, expiration, current_timestamp)
    }
}
