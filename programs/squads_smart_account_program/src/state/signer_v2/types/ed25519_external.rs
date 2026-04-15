use anchor_lang::prelude::*;
use super::{ExternalSignerData, SessionKeyData};

// ============================================================================
// Ed25519 External Signer Data
// ============================================================================

/// Ed25519 external signer data for hardware keys or off-chain Ed25519 signers.
///
/// This is for Ed25519 keys that are NOT native Solana transaction signers.
/// Instead, they're verified via the Ed25519 precompile introspection.
///
/// ## Fields
/// - `external_pubkey`: 32 bytes - Ed25519 public key verified via precompile
/// - `session_key`: Session key data for temporary native key delegation
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct Ed25519ExternalData {
    pub external_pubkey: [u8; 32],
    pub session_key_data: SessionKeyData,
}

impl Default for Ed25519ExternalData {
    fn default() -> Self {
        Self {
            external_pubkey: [0u8; 32],
            session_key_data: SessionKeyData::default(),
        }
    }
}

impl Ed25519ExternalData {
    pub const SIZE: usize = 32 + SessionKeyData::SIZE; // 72 bytes
    pub const PACKED_PAYLOAD_LEN: usize = 1 + Self::SIZE + 8; // permissions + data + nonce

}

impl ExternalSignerData for Ed25519ExternalData {
    #[inline]
    fn session_key_data(&self) -> &SessionKeyData {
        &self.session_key_data
    }

    #[inline]
    fn session_key_data_mut(&mut self) -> &mut SessionKeyData {
        &mut self.session_key_data
    }
}
