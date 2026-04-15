use anchor_lang::prelude::*;
use super::{ExternalSignerData, SessionKeyData};

// ============================================================================
// P256/WebAuthn Signer Data
// ============================================================================

/// P256/WebAuthn signer data for passkey authentication.
///
/// ## Fields
/// - `compressed_pubkey`: 33 bytes - Compressed P256 public key for signature verification
/// - `rp_id_len`: 1 byte - Actual length of RP ID (since rp_id is zero-padded to 32 bytes)
/// - `rp_id`: 32 bytes - Relying Party ID string, used for origin verification
/// - `rp_id_hash`: 32 bytes - SHA256 of RP ID, provided by authenticator in auth data
/// - `counter`: 8 bytes - WebAuthn counter for replay protection (MUST be monotonically increasing)
/// - `session_key`: Session key data for temporary native key delegation
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct P256WebauthnData {
    pub compressed_pubkey: [u8; 33],
    pub rp_id_len: u8,
    pub rp_id: [u8; 32],
    pub rp_id_hash: [u8; 32],
    pub counter: u64,
    /// Session key for temporary native key delegation
    pub session_key_data: SessionKeyData,
}

impl Default for P256WebauthnData {
    fn default() -> Self {
        Self {
            compressed_pubkey: [0u8; 33],
            rp_id_len: 0,
            rp_id: [0u8; 32],
            rp_id_hash: [0u8; 32],
            counter: 0,
            session_key_data: SessionKeyData::default(),
        }
    }
}

impl P256WebauthnData {
    pub const SIZE: usize = 33 + 1 + 32 + 32 + 8 + SessionKeyData::SIZE; // 146 bytes
    pub const PACKED_PAYLOAD_LEN: usize = 1 + Self::SIZE + 8; // permissions + data + nonce

    /// Create new P256WebauthnData with RP ID (no session key).
    ///
    /// Note: The caller should verify that `rp_id_hash == sha256(rp_id)` before calling.
    pub fn new(compressed_pubkey: [u8; 33], rp_id: &[u8], rp_id_hash: [u8; 32], counter: u64) -> Self {
        let rp_id_len = rp_id.len().min(32) as u8;
        let mut rp_id_padded = [0u8; 32];
        rp_id_padded[..rp_id_len as usize].copy_from_slice(&rp_id[..rp_id_len as usize]);

        Self {
            compressed_pubkey,
            rp_id_len,
            rp_id: rp_id_padded,
            rp_id_hash,
            counter,
            session_key_data: SessionKeyData::default(),
        }
    }

    /// Get the RP ID as a slice (without padding)
    #[inline]
    pub fn get_rp_id(&self) -> &[u8] {
        &self.rp_id[..self.rp_id_len as usize]
    }

}

impl ExternalSignerData for P256WebauthnData {
    #[inline]
    fn session_key_data(&self) -> &SessionKeyData {
        &self.session_key_data
    }

    #[inline]
    fn session_key_data_mut(&mut self) -> &mut SessionKeyData {
        &mut self.session_key_data
    }
}
