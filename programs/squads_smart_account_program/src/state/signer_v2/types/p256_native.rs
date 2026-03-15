use anchor_lang::prelude::*;
use super::{ExternalSignerData, SessionKeyData};

// ============================================================================
// P256 Native Signer Data
// ============================================================================

/// P256 native signer data for raw secp256r1 signatures (non-WebAuthn).
///
/// Unlike P256Webauthn, this signer signs raw message hashes directly
/// without WebAuthn authenticatorData/clientDataJSON wrapping.
/// Useful for Apple Secure Enclave (CryptoKit), non-browser P256 signers,
/// and any context where WebAuthn ceremony is unnecessary.
///
/// ## Fields
/// - `compressed_pubkey`: 33 bytes - Compressed P256 public key for signature verification
/// - `session_key_data`: Session key data for temporary native key delegation
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct P256NativeData {
    pub compressed_pubkey: [u8; 33],
    pub session_key_data: SessionKeyData,
}

impl Default for P256NativeData {
    fn default() -> Self {
        Self {
            compressed_pubkey: [0u8; 33],
            session_key_data: SessionKeyData::default(),
        }
    }
}

impl P256NativeData {
    pub const SIZE: usize = 33 + SessionKeyData::SIZE; // 73 bytes
    pub const PACKED_PAYLOAD_LEN: usize = 1 + Self::SIZE + 8; // permissions + data + nonce
}

impl ExternalSignerData for P256NativeData {
    #[inline]
    fn session_key_data(&self) -> &SessionKeyData {
        &self.session_key_data
    }

    #[inline]
    fn session_key_data_mut(&mut self) -> &mut SessionKeyData {
        &mut self.session_key_data
    }
}
