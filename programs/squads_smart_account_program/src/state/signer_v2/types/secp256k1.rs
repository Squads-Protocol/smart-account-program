use anchor_lang::prelude::*;
use super::{ExternalSignerData, SessionKeyData};

// ============================================================================
// Secp256k1 Signer Data
// ============================================================================

/// Secp256k1 signer data for Ethereum-style authentication.
///
/// ## Fields
/// - `uncompressed_pubkey`: 64 bytes - Uncompressed secp256k1 public key (no 0x04 prefix)
/// - `eth_address`: 20 bytes - keccak256(pubkey)[12..32], the Ethereum address
/// - `has_eth_address`: 1 byte - Whether eth_address has been validated
/// - `session_key`: Session key data for temporary native key delegation
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct Secp256k1Data {
    pub uncompressed_pubkey: [u8; 64],
    pub eth_address: [u8; 20],
    pub has_eth_address: bool,
    pub session_key_data: SessionKeyData,
}

impl Default for Secp256k1Data {
    fn default() -> Self {
        Self {
            uncompressed_pubkey: [0u8; 64],
            eth_address: [0u8; 20],
            has_eth_address: false,
            session_key_data: SessionKeyData::default(),
        }
    }
}

impl Secp256k1Data {
    pub const SIZE: usize = 64 + 20 + 1 + SessionKeyData::SIZE; // 125 bytes
    pub const PACKED_PAYLOAD_LEN: usize = 1 + Self::SIZE + 8; // permissions + data + nonce

}

impl ExternalSignerData for Secp256k1Data {
    #[inline]
    fn session_key_data(&self) -> &SessionKeyData {
        &self.session_key_data
    }

    #[inline]
    fn session_key_data_mut(&mut self) -> &mut SessionKeyData {
        &mut self.session_key_data
    }
}
