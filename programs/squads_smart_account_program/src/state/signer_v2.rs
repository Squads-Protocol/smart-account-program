use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};
use std::io::{Read, Write};

// Re-export from settings for convenience
pub use super::settings::{Permission, Permissions, LegacySmartAccountSigner};

// ============================================================================
// Constants
// ============================================================================

/// Maximum session key expiration: 3 months (in seconds)
/// This matches the external-signature-program's SESSION_KEY_EXPIRATION_LIMIT
pub const SESSION_KEY_EXPIRATION_LIMIT: u64 = 3 * 30 * 24 * 60 * 60; // ~7,776,000 seconds

/// Maximum number of signers. Limited to u16::MAX to ensure the 4-byte length
/// header [len_lo, len_mid, len_hi, version] doesn't overflow into the version byte.
pub const MAX_SIGNERS: usize = u16::MAX as usize; // 65535

/// WebAuthn authenticator data minimum size: rpIdHash(32) + flags(1) + counter(4) = 37 bytes
pub const WEBAUTHN_AUTH_DATA_MIN_SIZE: usize = 32 + 1 + 4;

/// WebAuthn clientDataJSON hash size (SHA256)
pub const WEBAUTHN_CLIENT_DATA_HASH_SIZE: usize = 32;

/// WebAuthn minimum signature payload size (auth data + clientDataHash)
pub const WEBAUTHN_SIGNATURE_MIN_SIZE: usize = WEBAUTHN_AUTH_DATA_MIN_SIZE + WEBAUTHN_CLIENT_DATA_HASH_SIZE; // 69 bytes

// ============================================================================
// V2 Signer Type
// ============================================================================

/// V2 signer type discriminator (explicit u8 values for stability)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SignerType {
    Native = 0,
    P256Webauthn = 1,
    Secp256k1 = 2,
    Ed25519External = 3,
}

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
    pub const PACKED_PAYLOAD_LEN: usize = 33 + Self::SIZE;

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

    #[inline]
    fn session_key_data(&self) -> &SessionKeyData {
        &self.session_key_data
    }

    #[inline]
    fn session_key_data_mut(&mut self) -> &mut SessionKeyData {
        &mut self.session_key_data
    }

    /// Check if session key is active (not default and not expired)
    #[inline]
    pub fn has_active_session_key(&self, current_timestamp: u64) -> bool {
        self.session_key_data().is_active(current_timestamp)
    }

    /// Clear session key
    #[inline]
    pub fn clear_session_key(&mut self) {
        self.session_key_data_mut().clear();
    }

    /// Set session key with validation
    pub fn set_session_key(&mut self, key: Pubkey, expiration: u64, current_timestamp: u64) -> Result<()> {
        self.session_key_data_mut().set(key, expiration, current_timestamp)
    }
}

/// Parameters for reconstructing clientDataJSON on-chain
/// Packed into a single byte + optional port to minimize storage
#[derive(
    AnchorSerialize,
    AnchorDeserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Default,
)]
pub struct ClientDataJsonReconstructionParams {
    /// High 4 bits: type (0x00 = create, 0x10 = get)
    /// Low 4 bits: flags (cross_origin, http, google_extra)
    pub type_and_flags: u8,
    /// Optional port number (0 means no port)
    pub port: u16,
}

impl ClientDataJsonReconstructionParams {
    pub const TYPE_CREATE: u8 = 0x00;
    pub const TYPE_GET: u8 = 0x10;
    pub const FLAG_CROSS_ORIGIN: u8 = 0x01;
    pub const FLAG_HTTP_ORIGIN: u8 = 0x02;
    pub const FLAG_GOOGLE_EXTRA: u8 = 0x04;

    pub fn new(
        is_create: bool,
        cross_origin: bool,
        http_origin: bool,
        google_extra: bool,
        port: Option<u16>,
    ) -> Self {
        let type_bits = if is_create { Self::TYPE_CREATE } else { Self::TYPE_GET };
        let mut flags = 0u8;
        if cross_origin { flags |= Self::FLAG_CROSS_ORIGIN; }
        if http_origin { flags |= Self::FLAG_HTTP_ORIGIN; }
        if google_extra { flags |= Self::FLAG_GOOGLE_EXTRA; }

        Self {
            type_and_flags: type_bits | flags,
            port: port.unwrap_or(0),
        }
    }

    pub fn is_create(&self) -> bool {
        (self.type_and_flags & 0xF0) == Self::TYPE_CREATE
    }

    pub fn is_cross_origin(&self) -> bool {
        (self.type_and_flags & Self::FLAG_CROSS_ORIGIN) != 0
    }

    pub fn is_http(&self) -> bool {
        (self.type_and_flags & Self::FLAG_HTTP_ORIGIN) != 0
    }

    pub fn has_google_extra(&self) -> bool {
        (self.type_and_flags & Self::FLAG_GOOGLE_EXTRA) != 0
    }

    pub fn get_port(&self) -> Option<u16> {
        if self.port == 0 { None } else { Some(self.port) }
    }
}

// TODO: Check this for the message: https://github.com/Squads-Grid/external-signature-program/blob/0a6d1afd2aba79cb4f6356a4ad0d7fbcec0f887a/src/state/p256_webauthn/trait_impl.rs#L119


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
    pub const PACKED_PAYLOAD_LEN: usize = 33 + Self::SIZE;

    #[inline]
    fn session_key_data(&self) -> &SessionKeyData {
        &self.session_key_data
    }

    #[inline]
    fn session_key_data_mut(&mut self) -> &mut SessionKeyData {
        &mut self.session_key_data
    }

    /// Check if session key is active (not default and not expired)
    #[inline]
    pub fn has_active_session_key(&self, current_timestamp: u64) -> bool {
        self.session_key_data().is_active(current_timestamp)
    }

    /// Clear session key
    #[inline]
    pub fn clear_session_key(&mut self) {
        self.session_key_data_mut().clear();
    }

    /// Set session key with validation
    pub fn set_session_key(&mut self, key: Pubkey, expiration: u64, current_timestamp: u64) -> Result<()> {
        self.session_key_data_mut().set(key, expiration, current_timestamp)
    }
}

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
    pub const PACKED_PAYLOAD_LEN: usize = 33 + Self::SIZE;

    #[inline]
    fn session_key_data(&self) -> &SessionKeyData {
        &self.session_key_data
    }

    #[inline]
    fn session_key_data_mut(&mut self) -> &mut SessionKeyData {
        &mut self.session_key_data
    }

    /// Check if session key is active (not default and not expired)
    #[inline]
    pub fn has_active_session_key(&self, current_timestamp: u64) -> bool {
        self.session_key_data().is_active(current_timestamp)
    }

    /// Clear session key
    #[inline]
    pub fn clear_session_key(&mut self) {
        self.session_key_data_mut().clear();
    }

    /// Set session key with validation
    pub fn set_session_key(&mut self, key: Pubkey, expiration: u64, current_timestamp: u64) -> Result<()> {
        self.session_key_data_mut().set(key, expiration, current_timestamp)
    }
}

// ============================================================================
// Unified V2 Signer Enum
// ============================================================================

/// Unified V2 signer enum
/// Each variant contains:
/// - key_id: Pubkey (deterministically derived from signer type + public key)
/// - permissions: Permissions (same bitmask as V1)
/// - type-specific data
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub enum SmartAccountSigner {
    /// Native Solana Ed25519 signer (verified via AccountInfo.is_signer)
    Native {
        key: Pubkey,
        permissions: Permissions,
    },

    /// P256/WebAuthn passkey signer (verified via secp256r1 precompile introspection)
    P256Webauthn {
        key_id: Pubkey,
        permissions: Permissions,
        data: P256WebauthnData,
    },

    /// Secp256k1/Ethereum-style signer (verified via secp256k1 precompile introspection)
    Secp256k1 {
        key_id: Pubkey,
        permissions: Permissions,
        data: Secp256k1Data,
    },

    /// Ed25519 external signer (verified via ed25519 precompile introspection, NOT native Signer)
    Ed25519External {
        key_id: Pubkey,
        permissions: Permissions,
        data: Ed25519ExternalData,
    },

    // TODO: scrap key_id just use a convention that is the uncompressed key [..32] or whatever

    // TODO: P256-NATIVE to use it with the precompile.

    // TODO: add nonce in each of them.
}

impl SmartAccountSigner {
    /// Derive deterministic key_id from signer type and canonical public key bytes
    pub fn derive_key_id(signer_type: SignerType, canonical_key: &[u8]) -> Pubkey {
        use anchor_lang::solana_program::hash::hash;

        let mut data = Vec::with_capacity(1 + canonical_key.len());
        data.push(signer_type as u8);
        data.extend_from_slice(canonical_key);

        Pubkey::new_from_array(hash(&data).to_bytes())
    }

    /// Create a SmartAccountSigner from raw instruction data.
    ///
    /// # Arguments
    /// - `signer_type`: The type of signer (0=Native, 1=P256Webauthn, 2=Secp256k1, 3=Ed25519External)
    /// - `key`: For Native signers, the signer's pubkey. Ignored for external signers (key_id is derived).
    /// - `permissions`: The permissions for this signer
    /// - `signer_data`: Signer-specific data:
    ///   - Native: empty (0 bytes)
    ///   - P256Webauthn: 74 bytes (compressed_pubkey(33) + rp_id_len(1) + rp_id(32) + counter(8))
    ///     Note: rp_id_hash is derived from rp_id, not provided by caller
    ///   - Secp256k1: 85 bytes (uncompressed_pubkey(64) + eth_address(20) + has_eth_address(1))
    ///   - Ed25519External: 32 bytes (external_pubkey)
    pub fn from_raw_data(
        signer_type: u8,
        key: Pubkey,
        permissions: Permissions,
        signer_data: &[u8],
    ) -> Result<Self> {
        // Validate permissions mask (must be < 8, only bits 0-2 are valid)
        if permissions.mask >= 8 {
            return Err(error!(crate::errors::SmartAccountError::InvalidPermissions));
        }

        let signer_type = match signer_type {
            0 => SignerType::Native,
            1 => SignerType::P256Webauthn,
            2 => SignerType::Secp256k1,
            3 => SignerType::Ed25519External,
            _ => return Err(error!(crate::errors::SmartAccountError::InvalidSignerType)),
        };

        match signer_type {
            SignerType::Native => {
                // Native signers must not have default pubkey
                if key == Pubkey::default() {
                    return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
                }
                // Native signers have no additional data, key is used directly
                if !signer_data.is_empty() {
                    return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
                }
                Ok(Self::Native { key, permissions })
            }
            SignerType::P256Webauthn => {
                // Layout: compressed_pubkey(33) + rp_id_len(1) + rp_id(32) + counter(8) = 74 bytes
                // Note: rp_id_hash is computed from rp_id, not accepted from user input
                if signer_data.len() != 74 {
                    return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
                }
                let mut compressed_pubkey = [0u8; 33];
                compressed_pubkey.copy_from_slice(&signer_data[0..33]);

                let rp_id_len = signer_data[33];
                if rp_id_len > 32 {
                    return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
                }

                let mut rp_id = [0u8; 32];
                rp_id.copy_from_slice(&signer_data[34..66]);

                // Derive rp_id_hash from rp_id (don't trust user input)
                // This matches the external-signature-program approach
                use anchor_lang::solana_program::hash::hash;
                let rp_id_hash_result = hash(&rp_id[..rp_id_len as usize]);
                let mut rp_id_hash = [0u8; 32];
                rp_id_hash.copy_from_slice(&rp_id_hash_result.to_bytes());

                let counter = u64::from_le_bytes(
                    signer_data[66..74]
                        .try_into()
                        .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
                );

                // Derive key_id deterministically from the compressed public key
                let key_id = Self::derive_key_id(SignerType::P256Webauthn, &compressed_pubkey);

                Ok(Self::P256Webauthn {
                    key_id,
                    permissions,
                    data: P256WebauthnData {
                        compressed_pubkey,
                        rp_id_len,
                        rp_id,
                        rp_id_hash,
                        counter,
                        session_key_data: SessionKeyData::default(),
                    },
                })
            }
            SignerType::Secp256k1 => {
                // Layout: uncompressed_pubkey(64) + eth_address(20) + has_eth_address(1) = 85 bytes
                if signer_data.len() != 85 {
                    return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
                }
                let mut uncompressed_pubkey = [0u8; 64];
                uncompressed_pubkey.copy_from_slice(&signer_data[0..64]);
                let mut eth_address = [0u8; 20];
                eth_address.copy_from_slice(&signer_data[64..84]);
                let has_eth_address = signer_data[84] != 0;

                // Derive key_id deterministically from the uncompressed public key
                let key_id = Self::derive_key_id(SignerType::Secp256k1, &uncompressed_pubkey);

                Ok(Self::Secp256k1 {
                    key_id,
                    permissions,
                    data: Secp256k1Data {
                        uncompressed_pubkey,
                        eth_address,
                        has_eth_address,
                        session_key_data: SessionKeyData::default(),
                    },
                })
            }
            SignerType::Ed25519External => {
                // Layout: external_pubkey(32) = 32 bytes
                if signer_data.len() != 32 {
                    return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
                }
                let mut external_pubkey = [0u8; 32];
                external_pubkey.copy_from_slice(&signer_data[0..32]);

                // Derive key_id deterministically from the external public key
                let key_id = Self::derive_key_id(SignerType::Ed25519External, &external_pubkey);

                Ok(Self::Ed25519External {
                    key_id,
                    permissions,
                    data: Ed25519ExternalData {
                        external_pubkey,
                        session_key_data: SessionKeyData::default(),
                    },
                })
            }
        }
    }

    /// Get the key (Native) or key_id (External) for this signer
    pub fn key(&self) -> Pubkey {
        match self {
            Self::Native { key, .. } => *key,
            Self::P256Webauthn { key_id, .. } => *key_id,
            Self::Secp256k1 { key_id, .. } => *key_id,
            Self::Ed25519External { key_id, .. } => *key_id,
        }
    }

    /// Get permissions for this signer
    pub fn permissions(&self) -> Permissions {
        match self {
            Self::Native { permissions, .. } => *permissions,
            Self::P256Webauthn { permissions, .. } => *permissions,
            Self::Secp256k1 { permissions, .. } => *permissions,
            Self::Ed25519External { permissions, .. } => *permissions,
        }
    }

    /// Get signer type discriminator
    pub fn signer_type(&self) -> SignerType {
        match self {
            Self::Native { .. } => SignerType::Native,
            Self::P256Webauthn { .. } => SignerType::P256Webauthn,
            Self::Secp256k1 { .. } => SignerType::Secp256k1,
            Self::Ed25519External { .. } => SignerType::Ed25519External,
        }
    }

    /// Check if this is a native Solana signer
    pub fn is_native(&self) -> bool {
        matches!(self, Self::Native { .. })
    }

    /// Check if this is an external signer (requires precompile introspection)
    pub fn is_external(&self) -> bool {
        !self.is_native()
    }

    /// Get the raw public key bytes for signature verification
    pub fn get_public_key_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Native { .. } => None,
            Self::P256Webauthn { data, .. } => Some(&data.compressed_pubkey),
            Self::Secp256k1 { data, .. } => Some(&data.uncompressed_pubkey),
            Self::Ed25519External { data, .. } => Some(&data.external_pubkey),
        }
    }

    /// Check if two signers have the same underlying public key (not just key_id)
    /// This prevents adding the same external public key with different key_ids
    pub fn has_same_public_key(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Native { key: k1, .. }, Self::Native { key: k2, .. }) => k1 == k2,
            (Self::P256Webauthn { data: d1, .. }, Self::P256Webauthn { data: d2, .. }) => {
                d1.compressed_pubkey == d2.compressed_pubkey
            }
            (Self::Secp256k1 { data: d1, .. }, Self::Secp256k1 { data: d2, .. }) => {
                d1.uncompressed_pubkey == d2.uncompressed_pubkey
            }
            (Self::Ed25519External { data: d1, .. }, Self::Ed25519External { data: d2, .. }) => {
                d1.external_pubkey == d2.external_pubkey
            }
            // Different signer types can't have the same key
            _ => false,
        }
    }

    /// Get the session key for this signer (if external and has one)
    pub fn get_session_key(&self) -> Option<Pubkey> {
        match self {
            Self::Native { .. } => None,
            Self::P256Webauthn { data, .. } => {
                if data.session_key_data.key != Pubkey::default() {
                    Some(data.session_key_data.key)
                } else {
                    None
                }
            }
            Self::Secp256k1 { data, .. } => {
                if data.session_key_data.key != Pubkey::default() {
                    Some(data.session_key_data.key)
                } else {
                    None
                }
            }
            Self::Ed25519External { data, .. } => {
                if data.session_key_data.key != Pubkey::default() {
                    Some(data.session_key_data.key)
                } else {
                    None
                }
            }
        }
    }

    /// Check if session key is active (exists and not expired)
    pub fn has_active_session_key(&self, current_timestamp: u64) -> bool {
        match self {
            Self::Native { .. } => false,
            Self::P256Webauthn { data, .. } => data.has_active_session_key(current_timestamp),
            Self::Secp256k1 { data, .. } => data.has_active_session_key(current_timestamp),
            Self::Ed25519External { data, .. } => data.has_active_session_key(current_timestamp),
        }
    }

    /// Check if a given pubkey matches this signer's session key and is active
    pub fn is_valid_session_key(&self, pubkey: &Pubkey, current_timestamp: u64) -> bool {
        match self {
            Self::Native { .. } => false,
            Self::P256Webauthn { data, .. } => {
                data.session_key_data.key == *pubkey && data.has_active_session_key(current_timestamp)
            }
            Self::Secp256k1 { data, .. } => {
                data.session_key_data.key == *pubkey && data.has_active_session_key(current_timestamp)
            }
            Self::Ed25519External { data, .. } => {
                data.session_key_data.key == *pubkey && data.has_active_session_key(current_timestamp)
            }
        }
    }

    /// Set session key (only for external signers)
    pub fn set_session_key(&mut self, key: Pubkey, expiration: u64, current_timestamp: u64) -> Result<()> {
        match self {
            Self::Native { .. } => Err(error!(crate::errors::SmartAccountError::InvalidSignerType)),
            Self::P256Webauthn { data, .. } => data.set_session_key(key, expiration, current_timestamp),
            Self::Secp256k1 { data, .. } => data.set_session_key(key, expiration, current_timestamp),
            Self::Ed25519External { data, .. } => data.set_session_key(key, expiration, current_timestamp),
        }
    }

    /// Clear session key (only for external signers)
    pub fn clear_session_key(&mut self) -> Result<()> {
        match self {
            Self::Native { .. } => Err(error!(crate::errors::SmartAccountError::InvalidSignerType)),
            Self::P256Webauthn { data, .. } => {
                data.clear_session_key();
                Ok(())
            }
            Self::Secp256k1 { data, .. } => {
                data.clear_session_key();
                Ok(())
            }
            Self::Ed25519External { data, .. } => {
                data.clear_session_key();
                Ok(())
            }
        }
    }

    /// Update WebAuthn counter (only for P256Webauthn signers)
    pub fn update_counter(&mut self, new_counter: u64) -> Result<()> {
        match self {
            Self::P256Webauthn { data, .. } => {
                data.counter = new_counter;
                Ok(())
            }
            _ => Err(error!(crate::errors::SmartAccountError::InvalidSignerType)),
        }
    }

    /// Get WebAuthn counter (only for P256Webauthn signers)
    pub fn get_counter(&self) -> Option<u64> {
        match self {
            Self::P256Webauthn { data, .. } => Some(data.counter),
            _ => None,
        }
    }

    /// Convert from V1 LegacySmartAccountSigner (always Native)
    pub fn from_v1(signer: &LegacySmartAccountSigner) -> Self {
        Self::Native {
            key: signer.key,
            permissions: signer.permissions,
        }
    }

    /// Convert to V1 LegacySmartAccountSigner (only if Native)
    pub fn to_v1(&self) -> Option<LegacySmartAccountSigner> {
        match self {
            Self::Native { key, permissions } => Some(LegacySmartAccountSigner {
                key: *key,
                permissions: *permissions,
            }),
            _ => None,
        }
    }

    /// Payload size for packed encoding (without the 4-byte header)
    pub fn packed_payload_size(&self) -> usize {
        match self {
            Self::Native { .. } => 33,                              // 32 (key) + 1 (permissions)
            Self::P256Webauthn { .. } => P256WebauthnData::PACKED_PAYLOAD_LEN,
            Self::Secp256k1 { .. } => Secp256k1Data::PACKED_PAYLOAD_LEN,
            Self::Ed25519External { .. } => Ed25519ExternalData::PACKED_PAYLOAD_LEN,
        }
    }

    /// Convert to packed payload bytes (V2 on-chain encoding)
    /// Returns (tag, payload_bytes)
    pub fn to_packed_payload(&self) -> (u8, Vec<u8>) {
        match self {
            Self::Native { key, permissions } => {
                let mut payload = Vec::with_capacity(33);
                payload.extend_from_slice(key.as_ref());
                payload.push(permissions.mask);
                (SignerType::Native as u8, payload)
            }
            Self::P256Webauthn {
                key_id,
                permissions,
                data,
            } => {
                let mut payload = Vec::with_capacity(P256WebauthnData::PACKED_PAYLOAD_LEN);
                payload.extend_from_slice(key_id.as_ref());
                payload.push(permissions.mask);
                payload.extend_from_slice(&data.compressed_pubkey);
                payload.push(data.rp_id_len);
                payload.extend_from_slice(&data.rp_id);
                payload.extend_from_slice(&data.rp_id_hash);
                payload.extend_from_slice(&data.counter.to_le_bytes());
                payload.extend_from_slice(data.session_key_data.key.as_ref());
                payload.extend_from_slice(&data.session_key_data.expiration.to_le_bytes());
                (SignerType::P256Webauthn as u8, payload)
            }
            Self::Secp256k1 {
                key_id,
                permissions,
                data,
            } => {
                let mut payload = Vec::with_capacity(Secp256k1Data::PACKED_PAYLOAD_LEN);
                payload.extend_from_slice(key_id.as_ref());
                payload.push(permissions.mask);
                payload.extend_from_slice(&data.uncompressed_pubkey);
                payload.extend_from_slice(&data.eth_address);
                payload.push(data.has_eth_address as u8);
                payload.extend_from_slice(data.session_key_data.key.as_ref());
                payload.extend_from_slice(&data.session_key_data.expiration.to_le_bytes());
                (SignerType::Secp256k1 as u8, payload)
            }
            Self::Ed25519External {
                key_id,
                permissions,
                data,
            } => {
                let mut payload = Vec::with_capacity(Ed25519ExternalData::PACKED_PAYLOAD_LEN);
                payload.extend_from_slice(key_id.as_ref());
                payload.push(permissions.mask);
                payload.extend_from_slice(&data.external_pubkey);
                payload.extend_from_slice(data.session_key_data.key.as_ref());
                payload.extend_from_slice(&data.session_key_data.expiration.to_le_bytes());
                (SignerType::Ed25519External as u8, payload)
            }
        }
    }

    /// Parse from packed Native payload (33 bytes: key + permissions)
    pub fn from_packed_native(payload: &[u8]) -> Result<Self> {
        if payload.len() != 33 {
            return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
        }
        let key = Pubkey::try_from(&payload[..32])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;
        let permissions = Permissions { mask: payload[32] };
        Ok(Self::Native { key, permissions })
    }

    /// Parse from packed P256Webauthn payload (179 bytes)
    pub fn from_packed_p256(payload: &[u8]) -> Result<Self> {
        if payload.len() != P256WebauthnData::PACKED_PAYLOAD_LEN {
            return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
        }
        let key_id = Pubkey::try_from(&payload[..32])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;
        let permissions = Permissions { mask: payload[32] };

        let mut compressed_pubkey = [0u8; 33];
        compressed_pubkey.copy_from_slice(&payload[33..66]);

        let rp_id_len = payload[66];

        let mut rp_id = [0u8; 32];
        rp_id.copy_from_slice(&payload[67..99]);

        let mut rp_id_hash = [0u8; 32];
        rp_id_hash.copy_from_slice(&payload[99..131]);

        let counter = u64::from_le_bytes(
            payload[131..139]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        let session_key = Pubkey::try_from(&payload[139..171])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;

        let session_key_expiration = u64::from_le_bytes(
            payload[171..179]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        Ok(Self::P256Webauthn {
            key_id,
            permissions,
            data: P256WebauthnData {
                compressed_pubkey,
                rp_id_len,
                rp_id,
                rp_id_hash,
                counter,
                session_key_data: SessionKeyData {
                    key: session_key,
                    expiration: session_key_expiration,
                },
            },
        })
    }

    /// Parse from packed Secp256k1 payload (158 bytes)
    pub fn from_packed_secp256k1(payload: &[u8]) -> Result<Self> {
        if payload.len() != Secp256k1Data::PACKED_PAYLOAD_LEN {
            return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
        }
        let key_id = Pubkey::try_from(&payload[..32])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;
        let permissions = Permissions { mask: payload[32] };

        let mut uncompressed_pubkey = [0u8; 64];
        uncompressed_pubkey.copy_from_slice(&payload[33..97]);

        let mut eth_address = [0u8; 20];
        eth_address.copy_from_slice(&payload[97..117]);

        let has_eth_address = payload[117] != 0;

        let session_key = Pubkey::try_from(&payload[118..150])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;

        let session_key_expiration = u64::from_le_bytes(
            payload[150..158]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        Ok(Self::Secp256k1 {
            key_id,
            permissions,
            data: Secp256k1Data {
                uncompressed_pubkey,
                eth_address,
                has_eth_address,
                session_key_data: SessionKeyData {
                    key: session_key,
                    expiration: session_key_expiration,
                },
            },
        })
    }

    /// Parse from packed Ed25519External payload (105 bytes)
    pub fn from_packed_ed25519_external(payload: &[u8]) -> Result<Self> {
        if payload.len() != Ed25519ExternalData::PACKED_PAYLOAD_LEN {
            return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
        }
        let key_id = Pubkey::try_from(&payload[..32])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;
        let permissions = Permissions { mask: payload[32] };

        let mut external_pubkey = [0u8; 32];
        external_pubkey.copy_from_slice(&payload[33..65]);

        let session_key = Pubkey::try_from(&payload[65..97])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;

        let session_key_expiration = u64::from_le_bytes(
            payload[97..105]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        Ok(Self::Ed25519External {
            key_id,
            permissions,
            data: Ed25519ExternalData {
                external_pubkey,
                session_key_data: SessionKeyData {
                    key: session_key,
                    expiration: session_key_expiration,
                },
            },
        })
    }
}

/// Version discriminator values (stored in byte 3 of Vec length)
pub const SIGNERS_VERSION_V1: u8 = 0x00;
pub const SIGNERS_VERSION_V2: u8 = 0x01;

/// Packed V2 entry header: <u8 tag><u16 payload_len LE><u8 flags>
pub const ENTRY_HEADER_LEN: usize = 4;

/// Wrapper for signers that supports both V1 and V2 formats
/// Uses custom Borsh serialization to maintain V1 backward compatibility
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SmartAccountSignerWrapper {
    V1(Vec<LegacySmartAccountSigner>),
    V2(Vec<SmartAccountSigner>),
}

impl Default for SmartAccountSignerWrapper {
    fn default() -> Self {
        Self::V1(Vec::new())
    }
}

impl SmartAccountSignerWrapper {
    /// Create a V1 wrapper from a Vec of LegacySmartAccountSigner
    pub fn from_v1_signers(signers: Vec<LegacySmartAccountSigner>) -> Self {
        Self::V1(signers)
    }

    /// Create a V2 wrapper from a Vec of SmartAccountSigner
    pub fn from_v2_signers(signers: Vec<SmartAccountSigner>) -> Self {
        Self::V2(signers)
    }

    pub fn version(&self) -> u8 {
        match self {
            Self::V1(_) => SIGNERS_VERSION_V1,
            Self::V2(_) => SIGNERS_VERSION_V2,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::V1(v) => v.len(),
            Self::V2(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a reference to the inner V1 signers slice.
    /// Returns the slice directly for V1, or panics for V2.
    ///
    /// IMPORTANT: This is for backward compatibility with the Consensus trait.
    /// For V2 accounts, use `as_v2()` or `as_v1_lossy()` instead.
    /// Get a reference to the inner V1 signers slice.
    ///
    /// Returns `Some(&[LegacySmartAccountSigner])` for V1 wrappers, `None` for V2.
    #[inline]
    pub fn try_as_v1_slice(&self) -> Option<&[LegacySmartAccountSigner]> {
        match self {
            Self::V1(signers) => Some(signers.as_slice()),
            Self::V2(_) => None,
        }
    }

    /// Get signers as V2 format (canonical view)
    pub fn as_v2(&self) -> Vec<SmartAccountSigner> {
        match self {
            Self::V1(signers) => signers.iter().map(SmartAccountSigner::from_v1).collect(),
            Self::V2(signers) => signers.clone(),
        }
    }

    /// Get signers as V1 format (only if all are Native)
    pub fn as_v1(&self) -> Option<Vec<LegacySmartAccountSigner>> {
        match self {
            Self::V1(signers) => Some(signers.clone()),
            Self::V2(signers) => {
                let v1_signers: Option<Vec<_>> =
                    signers.iter().map(|s| s.to_v1()).collect();
                v1_signers
            }
        }
    }

    /// Find a signer by key/key_id
    pub fn find(&self, key: &Pubkey) -> Option<SmartAccountSigner> {
        match self {
            Self::V1(signers) => signers
                .iter()
                .find(|s| &s.key == key)
                .map(SmartAccountSigner::from_v1),
            Self::V2(signers) => signers.iter().find(|s| &s.key() == key).cloned(),
        }
    }

    /// Find a mutable reference to a signer by key/key_id (V2 only).
    /// Returns None if the wrapper is V1 (since V1 signers don't have counters).
    pub fn find_mut(&mut self, key: &Pubkey) -> Option<&mut SmartAccountSigner> {
        match self {
            Self::V1(_) => None, // V1 signers don't have counters
            Self::V2(signers) => signers.iter_mut().find(|s| &s.key() == key),
        }
    }

    /// Update the WebAuthn counter for a signer by key_id.
    /// Returns Ok(()) if successful, Err if signer not found or not a WebAuthn signer.
    pub fn update_signer_counter(&mut self, key_id: &Pubkey, new_counter: u64) -> Result<()> {
        match self {
            Self::V1(_) => {
                // V1 signers are all Native, they don't have counters
                Err(error!(crate::errors::SmartAccountError::InvalidSignerType))
            }
            Self::V2(signers) => {
                let signer = signers
                    .iter_mut()
                    .find(|s| &s.key() == key_id)
                    .ok_or_else(|| error!(crate::errors::SmartAccountError::NotASigner))?;
                signer.update_counter(new_counter)
            }
        }
    }

    /// Apply multiple counter updates from WebAuthn signature verification.
    /// This is used after synchronous consensus validation to persist counter state.
    pub fn apply_counter_updates(&mut self, updates: &[(Pubkey, u64)]) -> Result<()> {
        for (key_id, new_counter) in updates {
            self.update_signer_counter(key_id, *new_counter)?;
        }
        Ok(())
    }

    /// Find index of a signer by key/key_id
    pub fn find_index(&self, key: &Pubkey) -> Option<usize> {
        match self {
            Self::V1(signers) => signers.iter().position(|s| &s.key == key),
            Self::V2(signers) => signers.iter().position(|s| &s.key() == key),
        }
    }

    /// Get signer at index
    pub fn get(&self, index: usize) -> Option<SmartAccountSigner> {
        match self {
            Self::V1(signers) => signers.get(index).map(SmartAccountSigner::from_v1),
            Self::V2(signers) => signers.get(index).cloned(),
        }
    }

    /// Push a V2 signer (converts to V2 format if needed)
    pub fn push_v2(&mut self, signer: SmartAccountSigner) {
        match self {
            Self::V1(signers) => {
                // If signer is Native, can stay V1
                if let Some(v1_signer) = signer.to_v1() {
                    signers.push(v1_signer);
                } else {
                    // Convert to V2
                    let mut v2_signers: Vec<SmartAccountSigner> =
                        signers.iter().map(SmartAccountSigner::from_v1).collect();
                    v2_signers.push(signer);
                    *self = Self::V2(v2_signers);
                }
            }
            Self::V2(signers) => {
                signers.push(signer);
            }
        }
    }

    /// Remove signer at index
    pub fn remove(&mut self, index: usize) -> SmartAccountSigner {
        match self {
            Self::V1(signers) => SmartAccountSigner::from_v1(&signers.remove(index)),
            Self::V2(signers) => signers.remove(index),
        }
    }

    /// Sort signers by key
    pub fn sort_by_key<F, K>(&mut self, mut f: F)
    where
        F: FnMut(&SmartAccountSigner) -> K,
        K: Ord,
    {
        match self {
            Self::V1(signers) => {
                signers.sort_by_key(|s| f(&SmartAccountSigner::from_v1(s)));
            }
            Self::V2(signers) => {
                signers.sort_by_key(|s| f(s));
            }
        }
    }

    /// Force V2 format (for when external signers are added)
    pub fn force_v2(&mut self) {
        if let Self::V1(signers) = self {
            let v2_signers: Vec<SmartAccountSigner> =
                signers.iter().map(SmartAccountSigner::from_v1).collect();
            *self = Self::V2(v2_signers);
        }
    }

    /// Check if wrapper contains any external signers
    pub fn has_external_signers(&self) -> bool {
        match self {
            Self::V1(_) => false,
            Self::V2(signers) => signers.iter().any(|s| s.is_external()),
        }
    }

    /// Count signers with a specific permission
    pub fn count_with_permission(&self, permission: Permission) -> usize {
        match self {
            Self::V1(signers) => signers.iter().filter(|s| s.permissions.has(permission)).count(),
            Self::V2(signers) => signers.iter().filter(|s| s.permissions().has(permission)).count(),
        }
    }

    /// Get the last signer
    pub fn last(&self) -> Option<SmartAccountSigner> {
        match self {
            Self::V1(signers) => signers.last().map(SmartAccountSigner::from_v1),
            Self::V2(signers) => signers.last().cloned(),
        }
    }

    /// Iterate over signers as V2 (zero allocation)
    pub fn iter_v2(&self) -> SignerIterator<'_> {
        SignerIterator {
            wrapper: self,
            index: 0,
        }
    }

    /// Calculate the serialized size in bytes
    /// Calculate the serialized size in bytes without allocating.
    pub fn serialized_size(&self) -> usize {
        match self {
            Self::V1(signers) => {
                // 4 bytes for length + 33 bytes per V1 signer
                4 + signers.len() * LegacySmartAccountSigner::INIT_SPACE
            }
            Self::V2(signers) => {
                // 4 bytes for length + (header + payload) per signer
                // Use packed_payload_size() to avoid allocating Vec for each signer
                4 + signers.iter()
                    .map(|s| ENTRY_HEADER_LEN + s.packed_payload_size())
                    .sum::<usize>()
            }
        }
    }

    /// Add a V1 signer (for backward compatibility)
    pub fn add_signer(&mut self, signer: LegacySmartAccountSigner) {
        self.push_v2(SmartAccountSigner::from_v1(&signer));
    }

    /// Add a V2 signer directly
    pub fn add_signer_v2(&mut self, signer: SmartAccountSigner) {
        self.push_v2(signer);
    }

    /// Remove signer by key and return it
    pub fn remove_signer(&mut self, key: &Pubkey) -> Option<SmartAccountSigner> {
        let index = self.find_index(key)?;
        Some(self.remove(index))
    }

    /// Sort signers by key (V2 key)
    pub fn sort_by_signer_key(&mut self) {
        self.sort_by_key(|s| s.key());
    }

    /// Check for duplicate signers by key/key_id
    pub fn has_duplicates(&self) -> bool {
        let mut seen: std::collections::BTreeSet<Pubkey> = std::collections::BTreeSet::new();
        self.iter_v2().any(|s| !seen.insert(s.key()))
    }

    /// Check if any existing signer has the same public key as the given signer.
    /// This prevents adding the same external public key with a different key_id.
    pub fn has_duplicate_public_key(&self, new_signer: &SmartAccountSigner) -> bool {
        let mut seen: std::collections::BTreeSet<Vec<u8>> = std::collections::BTreeSet::new();
        for existing in self.iter_v2() {
            let key_bytes: Vec<u8> = match existing {
                SmartAccountSigner::Native { key, .. } => key.to_bytes().to_vec(),
                SmartAccountSigner::P256Webauthn { data, .. } => data.compressed_pubkey.to_vec(),
                SmartAccountSigner::Secp256k1 { data, .. } => data.uncompressed_pubkey.to_vec(),
                SmartAccountSigner::Ed25519External { data, .. } => data.external_pubkey.to_vec(),
            };
            seen.insert(key_bytes);
        }

        let new_key_bytes: Vec<u8> = match new_signer {
            SmartAccountSigner::Native { key, .. } => key.to_bytes().to_vec(),
            SmartAccountSigner::P256Webauthn { data, .. } => data.compressed_pubkey.to_vec(),
            SmartAccountSigner::Secp256k1 { data, .. } => data.uncompressed_pubkey.to_vec(),
            SmartAccountSigner::Ed25519External { data, .. } => data.external_pubkey.to_vec(),
        };

        seen.contains(&new_key_bytes)
    }

    /// Find an external signer by their active session key.
    /// Returns the signer if the pubkey matches an active session key.
    pub fn find_by_session_key(&self, pubkey: &Pubkey, current_timestamp: u64) -> Option<SmartAccountSigner> {
        let signers = self.as_v2();
        signers
            .into_iter()
            .find(|s| s.is_valid_session_key(pubkey, current_timestamp))
    }

    /// Check if all signers have valid permissions (mask < 8)
    pub fn all_permissions_valid(&self) -> bool {
        match self {
            Self::V1(signers) => signers.iter().all(|s| s.permissions.mask < 8),
            Self::V2(signers) => signers.iter().all(|s| s.permissions().mask < 8),
        }
    }
}

/// Zero-allocation iterator over signers as SmartAccountSigner.
/// For V1 wrappers, converts each LegacySmartAccountSigner on the fly.
/// For V2 wrappers, clones each SmartAccountSigner.
pub struct SignerIterator<'a> {
    wrapper: &'a SmartAccountSignerWrapper,
    index: usize,
}

impl<'a> Iterator for SignerIterator<'a> {
    type Item = SmartAccountSigner;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.wrapper.get(self.index);
        if result.is_some() {
            self.index += 1;
        }
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.wrapper.len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for SignerIterator<'a> {}

/// Custom Borsh serialization: no enum discriminant on-chain
/// Emits: [len u32 LE with version in byte3] + signer bytes
impl BorshSerialize for SmartAccountSignerWrapper {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self {
            Self::V1(signers) => {
                // V1: Standard Vec<SmartAccountSigner> serialization
                // Length with version byte 0x00 in MSB
                let count = signers.len() as u32;
                let len_bytes = [
                    (count & 0xFF) as u8,
                    ((count >> 8) & 0xFF) as u8,
                    ((count >> 16) & 0xFF) as u8,
                    SIGNERS_VERSION_V1,
                ];
                writer.write_all(&len_bytes)?;

                // Each signer is 33 bytes (32 key + 1 permissions)
                for signer in signers {
                    signer.serialize(writer)?;
                }
            }
            Self::V2(signers) => {
                // V2: Custom packed format
                let count = signers.len() as u32;
                let len_bytes = [
                    (count & 0xFF) as u8,
                    ((count >> 8) & 0xFF) as u8,
                    ((count >> 16) & 0xFF) as u8,
                    SIGNERS_VERSION_V2,
                ];
                writer.write_all(&len_bytes)?;

                // Each entry: <u8 tag><u16 payload_len LE><u8 flags><payload>
                for signer in signers {
                    let (tag, payload) = signer.to_packed_payload();
                    writer.write_all(&[tag])?;
                    writer.write_all(&(payload.len() as u16).to_le_bytes())?;
                    writer.write_all(&[0u8])?; // flags (reserved)
                    writer.write_all(&payload)?;
                }
            }
        }
        Ok(())
    }
}

/// Custom Borsh deserialization
impl BorshDeserialize for SmartAccountSignerWrapper {
    fn deserialize_reader<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;

        let version = len_bytes[3];
        let count = u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], 0]) as usize;

        if count > MAX_SIGNERS {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Too many signers",
            ));
        }

        match version {
            SIGNERS_VERSION_V1 => {
                let mut signers = Vec::with_capacity(count);
                for _ in 0..count {
                    let signer = LegacySmartAccountSigner::deserialize_reader(reader)?;
                    signers.push(signer);
                }
                Ok(Self::V1(signers))
            }
            SIGNERS_VERSION_V2 => {
                let mut signers = Vec::with_capacity(count);
                for _ in 0..count {
                    let mut header = [0u8; ENTRY_HEADER_LEN];
                    reader.read_exact(&mut header)?;

                    let tag = header[0];
                    let payload_len = u16::from_le_bytes([header[1], header[2]]) as usize;
                    let _flags = header[3];

                    let mut payload = vec![0u8; payload_len];
                    reader.read_exact(&mut payload)?;

                    let signer = match tag {
                        x if x == SignerType::Native as u8 => {
                            SmartAccountSigner::from_packed_native(&payload)
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?
                        }
                        x if x == SignerType::P256Webauthn as u8 => {
                            SmartAccountSigner::from_packed_p256(&payload)
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?
                        }
                        x if x == SignerType::Secp256k1 as u8 => {
                            SmartAccountSigner::from_packed_secp256k1(&payload)
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?
                        }
                        x if x == SignerType::Ed25519External as u8 => {
                            SmartAccountSigner::from_packed_ed25519_external(&payload)
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?
                        }
                        _ => {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "Invalid signer type",
                            ));
                        }
                    };
                    signers.push(signer);
                }
                Ok(Self::V2(signers))
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Unsupported signer version",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v1_serialization_roundtrip() {
        let signers = vec![
            LegacySmartAccountSigner {
                key: Pubkey::new_unique(),
                permissions: Permissions::all(),
            },
            LegacySmartAccountSigner {
                key: Pubkey::new_unique(),
                permissions: Permissions { mask: 0b011 },
            },
        ];

        let wrapper = SmartAccountSignerWrapper::V1(signers.clone());
        let mut buf = Vec::new();
        wrapper.serialize(&mut buf).unwrap();

        // Verify version byte is 0x00
        assert_eq!(buf[3], SIGNERS_VERSION_V1);

        // Deserialize and verify
        let deserialized = SmartAccountSignerWrapper::deserialize(&mut buf.as_slice()).unwrap();
        assert_eq!(wrapper, deserialized);
    }

    #[test]
    fn test_v2_serialization_roundtrip() {
        let signers = vec![
            SmartAccountSigner::Native {
                key: Pubkey::new_unique(),
                permissions: Permissions::all(),
            },
            SmartAccountSigner::P256Webauthn {
                key_id: Pubkey::new_unique(),
                permissions: Permissions { mask: 0b111 },
                data: P256WebauthnData {
                    compressed_pubkey: [0x02; 33],
                    rp_id_len: 8,
                    rp_id: {
                        let mut arr = [0u8; 32];
                        arr[..8].copy_from_slice(b"test.com");
                        arr
                    },
                    rp_id_hash: [0xAB; 32],
                    counter: 42,
                    session_key_data: SessionKeyData::default(),
                },
            },
        ];

        let wrapper = SmartAccountSignerWrapper::V2(signers);
        let mut buf = Vec::new();
        wrapper.serialize(&mut buf).unwrap();

        // Verify version byte is 0x01
        assert_eq!(buf[3], SIGNERS_VERSION_V2);

        // Deserialize and verify
        let deserialized = SmartAccountSignerWrapper::deserialize(&mut buf.as_slice()).unwrap();
        assert_eq!(wrapper, deserialized);
    }

    #[test]
    fn test_derive_key_id() {
        let pubkey_bytes = [0x02; 33];
        let key_id = SmartAccountSigner::derive_key_id(SignerType::P256Webauthn, &pubkey_bytes);

        // Same input should produce same key_id
        let key_id_2 = SmartAccountSigner::derive_key_id(SignerType::P256Webauthn, &pubkey_bytes);
        assert_eq!(key_id, key_id_2);

        // Different type should produce different key_id
        let key_id_3 = SmartAccountSigner::derive_key_id(SignerType::Secp256k1, &pubkey_bytes);
        assert_ne!(key_id, key_id_3);
    }

    #[test]
    fn test_wrapper_force_v2() {
        let signers = vec![LegacySmartAccountSigner {
            key: Pubkey::new_unique(),
            permissions: Permissions::all(),
        }];

        let mut wrapper = SmartAccountSignerWrapper::V1(signers);
        assert_eq!(wrapper.version(), SIGNERS_VERSION_V1);

        wrapper.force_v2();
        assert_eq!(wrapper.version(), SIGNERS_VERSION_V2);
    }

    #[test]
    fn test_push_external_converts_to_v2() {
        let mut wrapper = SmartAccountSignerWrapper::V1(vec![LegacySmartAccountSigner {
            key: Pubkey::new_unique(),
            permissions: Permissions::all(),
        }]);

        assert_eq!(wrapper.version(), SIGNERS_VERSION_V1);

        // Push an external signer
        wrapper.push_v2(SmartAccountSigner::P256Webauthn {
            key_id: Pubkey::new_unique(),
            permissions: Permissions::all(),
            data: P256WebauthnData {
                compressed_pubkey: [0x02; 33],
                rp_id_len: 8,
                rp_id: {
                    let mut arr = [0u8; 32];
                    arr[..8].copy_from_slice(b"test.com");
                    arr
                },
                rp_id_hash: [0xAB; 32],
                counter: 0,
                session_key_data: SessionKeyData::default(),
            },
        });

        assert_eq!(wrapper.version(), SIGNERS_VERSION_V2);
        assert_eq!(wrapper.len(), 2);
    }

    // ========================================================================
    // Priority 1: V2 Signer Type Validation Tests
    // ========================================================================

    #[test]
    fn test_ed25519_external_data_size() {
        assert_eq!(Ed25519ExternalData::SIZE, 32 + 40); // pubkey + session key data
        assert_eq!(Ed25519ExternalData::PACKED_PAYLOAD_LEN, 33 + 72);
    }

    #[test]
    fn test_ed25519_external_serialization_roundtrip() {
        let data = Ed25519ExternalData {
            external_pubkey: [0xAB; 32],
            session_key_data: SessionKeyData {
                key: Pubkey::new_unique(),
                expiration: 1000000,
            },
        };

        let mut buf = Vec::new();
        data.serialize(&mut buf).unwrap();
        let deserialized = Ed25519ExternalData::deserialize(&mut buf.as_slice()).unwrap();

        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_secp256k1_data_size_validation() {
        assert_eq!(Secp256k1Data::SIZE, 64 + 20 + 1 + 40); // pubkey + eth_addr + has_eth + session
        assert_eq!(Secp256k1Data::PACKED_PAYLOAD_LEN, 33 + 125);
    }

    #[test]
    fn test_secp256k1_serialization_roundtrip() {
        let data = Secp256k1Data {
            uncompressed_pubkey: [0xCD; 64],
            eth_address: [0xEF; 20],
            has_eth_address: true,
            session_key_data: SessionKeyData::default(),
        };

        let mut buf = Vec::new();
        data.serialize(&mut buf).unwrap();
        let deserialized = Secp256k1Data::deserialize(&mut buf.as_slice()).unwrap();

        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_p256_webauthn_data_size_validation() {
        assert_eq!(P256WebauthnData::SIZE, 33 + 1 + 32 + 32 + 8 + 40); // 146 bytes
        assert_eq!(P256WebauthnData::PACKED_PAYLOAD_LEN, 33 + 146);
    }

    #[test]
    fn test_p256_rp_id_length_validation() {
        let data = P256WebauthnData::new(
            [0x02; 33],
            b"example.com", // 11 bytes
            [0xAB; 32],
            0,
        );

        assert_eq!(data.rp_id_len, 11);
        assert_eq!(data.get_rp_id(), b"example.com");
    }

    #[test]
    fn test_p256_rp_id_padding() {
        let long_rp_id = b"this-is-a-very-long-domain-name-that-exceeds-32-bytes-limit";
        let data = P256WebauthnData::new(
            [0x02; 33],
            long_rp_id,
            [0xAB; 32],
            0,
        );

        // Should be capped at 32 bytes
        assert_eq!(data.rp_id_len, 32);
        assert_eq!(data.get_rp_id().len(), 32);
    }

    #[test]
    fn test_p256_counter_increment() {
        let mut data = P256WebauthnData::default();
        assert_eq!(data.counter, 0);

        data.counter = 42;
        assert_eq!(data.counter, 42);

        data.counter = u64::MAX;
        assert_eq!(data.counter, u64::MAX);
    }

    #[test]
    fn test_session_key_expiration_validation() {
        let mut session_data = SessionKeyData::default();
        let current_time = 1000000;
        let key = Pubkey::new_unique();

        // Valid expiration (future)
        let future_expiration = current_time + 3600;
        assert!(session_data.set(key, future_expiration, current_time).is_ok());

        // Invalid: expiration equals current time
        let equal_expiration = current_time;
        assert!(session_data.set(key, equal_expiration, current_time).is_err());

        // Invalid: expiration in the past
        let past_expiration = current_time - 1;
        assert!(session_data.set(key, past_expiration, current_time).is_err());
    }

    #[test]
    fn test_session_key_expiration_limit() {
        let mut session_data = SessionKeyData::default();
        let current_time = 1000000;
        let key = Pubkey::new_unique();

        // Valid: exactly at the limit
        let max_expiration = current_time + SESSION_KEY_EXPIRATION_LIMIT;
        assert!(session_data.set(key, max_expiration, current_time).is_ok());

        // Invalid: exceeds the limit
        let over_limit = current_time + SESSION_KEY_EXPIRATION_LIMIT + 1;
        assert!(session_data.set(key, over_limit, current_time).is_err());
    }

    #[test]
    fn test_session_key_default_pubkey_rejection() {
        let mut session_data = SessionKeyData::default();
        let current_time = 1000000;
        let future_expiration = current_time + 3600;

        // Should reject default pubkey
        let result = session_data.set(Pubkey::default(), future_expiration, current_time);
        assert!(result.is_err());
    }

    #[test]
    fn test_session_key_is_active_boundary_cases() {
        let key = Pubkey::new_unique();
        let current_time = 1000000;

        // Active: not expired
        let active_session = SessionKeyData {
            key,
            expiration: current_time + 1,
        };
        assert!(active_session.is_active(current_time));

        // Inactive: exactly expired
        let expired_session = SessionKeyData {
            key,
            expiration: current_time,
        };
        assert!(!expired_session.is_active(current_time));

        // Inactive: default pubkey
        let default_session = SessionKeyData {
            key: Pubkey::default(),
            expiration: current_time + 1000,
        };
        assert!(!default_session.is_active(current_time));
    }

    #[test]
    fn test_signer_wrapper_v1_v2_compatibility() {
        let v1_signer = LegacySmartAccountSigner {
            key: Pubkey::new_unique(),
            permissions: Permissions::all(),
        };

        // Convert to V2
        let v2_signer = SmartAccountSigner::from_v1(&v1_signer);

        // Convert back to V1
        let back_to_v1 = v2_signer.to_v1().unwrap();

        assert_eq!(v1_signer.key, back_to_v1.key);
        assert_eq!(v1_signer.permissions, back_to_v1.permissions);
    }

    #[test]
    fn test_signer_wrapper_max_signers_validation() {
        // Test that MAX_SIGNERS is within u16::MAX
        assert_eq!(MAX_SIGNERS, u16::MAX as usize);

        // Test deserialization fails with too many signers
        let mut buf = Vec::new();
        let count = (MAX_SIGNERS + 1) as u32;
        let len_bytes = [
            (count & 0xFF) as u8,
            ((count >> 8) & 0xFF) as u8,
            ((count >> 16) & 0xFF) as u8,
            SIGNERS_VERSION_V1,
        ];
        buf.extend_from_slice(&len_bytes);

        let result = SmartAccountSignerWrapper::deserialize(&mut buf.as_slice());
        assert!(result.is_err());
    }

    #[test]
    fn test_signer_serialization_version_handling() {
        // V1 wrapper
        let v1_wrapper = SmartAccountSignerWrapper::V1(vec![
            LegacySmartAccountSigner {
                key: Pubkey::new_unique(),
                permissions: Permissions::all(),
            }
        ]);

        let mut v1_buf = Vec::new();
        v1_wrapper.serialize(&mut v1_buf).unwrap();
        assert_eq!(v1_buf[3], SIGNERS_VERSION_V1);

        // V2 wrapper
        let v2_wrapper = SmartAccountSignerWrapper::V2(vec![
            SmartAccountSigner::Native {
                key: Pubkey::new_unique(),
                permissions: Permissions::all(),
            }
        ]);

        let mut v2_buf = Vec::new();
        v2_wrapper.serialize(&mut v2_buf).unwrap();
        assert_eq!(v2_buf[3], SIGNERS_VERSION_V2);
    }

    #[test]
    fn test_from_raw_data_ed25519_size_validation() {
        let key = Pubkey::new_unique();
        let permissions = Permissions::all();

        // Valid: 32 bytes
        let valid_data = [0xAB; 32];
        let result = SmartAccountSigner::from_raw_data(3, key, permissions, &valid_data);
        assert!(result.is_ok());

        // Invalid: wrong size
        let invalid_data = [0xAB; 31];
        let result = SmartAccountSigner::from_raw_data(3, key, permissions, &invalid_data);
        assert!(result.is_err());

        let invalid_data = [0xAB; 33];
        let result = SmartAccountSigner::from_raw_data(3, key, permissions, &invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_raw_data_secp256k1_size_validation() {
        let key = Pubkey::new_unique();
        let permissions = Permissions::all();

        // Valid: 85 bytes (64 + 20 + 1)
        let mut valid_data = vec![0xAB; 64]; // uncompressed pubkey
        valid_data.extend_from_slice(&[0xCD; 20]); // eth address
        valid_data.push(1); // has_eth_address

        let result = SmartAccountSigner::from_raw_data(2, key, permissions, &valid_data);
        assert!(result.is_ok());

        // Invalid: wrong size
        let invalid_data = vec![0xAB; 84];
        let result = SmartAccountSigner::from_raw_data(2, key, permissions, &invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_raw_data_p256_size_validation() {
        let key = Pubkey::new_unique();
        let permissions = Permissions::all();

        // Valid: 74 bytes (33 + 1 + 32 + 8)
        let mut valid_data = vec![0x02; 33]; // compressed pubkey
        valid_data.push(11); // rp_id_len
        valid_data.extend_from_slice(b"example.com"); // rp_id (11 bytes)
        valid_data.extend_from_slice(&[0x00; 21]); // padding to 32 bytes
        valid_data.extend_from_slice(&42u64.to_le_bytes()); // counter

        let result = SmartAccountSigner::from_raw_data(1, key, permissions, &valid_data);
        assert!(result.is_ok());

        // Invalid: wrong size
        let invalid_data = vec![0x02; 73];
        let result = SmartAccountSigner::from_raw_data(1, key, permissions, &invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_raw_data_p256_rp_id_len_bounds() {
        let key = Pubkey::new_unique();
        let permissions = Permissions::all();

        // Valid: rp_id_len <= 32
        let mut valid_data = vec![0x02; 33];
        valid_data.push(32); // rp_id_len at max
        valid_data.extend_from_slice(&[0x41; 32]); // rp_id
        valid_data.extend_from_slice(&0u64.to_le_bytes()); // counter

        let result = SmartAccountSigner::from_raw_data(1, key, permissions, &valid_data);
        assert!(result.is_ok());

        // Invalid: rp_id_len > 32
        let mut invalid_data = vec![0x02; 33];
        invalid_data.push(33); // rp_id_len exceeds max
        invalid_data.extend_from_slice(&[0x41; 32]);
        invalid_data.extend_from_slice(&0u64.to_le_bytes());

        let result = SmartAccountSigner::from_raw_data(1, key, permissions, &invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_raw_data_native_empty_data() {
        let key = Pubkey::new_unique();
        let permissions = Permissions::all();

        // Valid: empty data for native
        let result = SmartAccountSigner::from_raw_data(0, key, permissions, &[]);
        assert!(result.is_ok());

        // Invalid: non-empty data for native
        let result = SmartAccountSigner::from_raw_data(0, key, permissions, &[0x00]);
        assert!(result.is_err());

        // Invalid: default pubkey for native
        let result = SmartAccountSigner::from_raw_data(0, Pubkey::default(), permissions, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_raw_data_invalid_permissions() {
        let key = Pubkey::new_unique();

        // Invalid permissions (mask >= 8)
        let invalid_permissions = Permissions { mask: 8 };
        let result = SmartAccountSigner::from_raw_data(0, key, invalid_permissions, &[]);
        assert!(result.is_err());

        let invalid_permissions = Permissions { mask: 255 };
        let result = SmartAccountSigner::from_raw_data(0, key, invalid_permissions, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_raw_data_invalid_signer_type() {
        let key = Pubkey::new_unique();
        let permissions = Permissions::all();

        // Invalid signer type (>= 4)
        let result = SmartAccountSigner::from_raw_data(4, key, permissions, &[]);
        assert!(result.is_err());

        let result = SmartAccountSigner::from_raw_data(255, key, permissions, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_has_same_public_key() {
        let pubkey1 = Pubkey::new_unique();
        let pubkey2 = Pubkey::new_unique();

        let native1 = SmartAccountSigner::Native {
            key: pubkey1,
            permissions: Permissions::all(),
        };

        let native2 = SmartAccountSigner::Native {
            key: pubkey1,
            permissions: Permissions { mask: 0b001 },
        };

        let native3 = SmartAccountSigner::Native {
            key: pubkey2,
            permissions: Permissions::all(),
        };

        // Same native key
        assert!(native1.has_same_public_key(&native2));

        // Different native keys
        assert!(!native1.has_same_public_key(&native3));

        // P256 signers with same compressed pubkey
        let p256_1 = SmartAccountSigner::P256Webauthn {
            key_id: Pubkey::new_unique(),
            permissions: Permissions::all(),
            data: P256WebauthnData {
                compressed_pubkey: [0x02; 33],
                rp_id_len: 8,
                rp_id: [0x00; 32],
                rp_id_hash: [0xAB; 32],
                counter: 0,
                session_key_data: SessionKeyData::default(),
            },
        };

        let p256_2 = SmartAccountSigner::P256Webauthn {
            key_id: Pubkey::new_unique(), // Different key_id
            permissions: Permissions::all(),
            data: P256WebauthnData {
                compressed_pubkey: [0x02; 33], // Same pubkey
                rp_id_len: 8,
                rp_id: [0x00; 32],
                rp_id_hash: [0xAB; 32],
                counter: 0,
                session_key_data: SessionKeyData::default(),
            },
        };

        // Same P256 pubkey
        assert!(p256_1.has_same_public_key(&p256_2));

        // Different signer types never match
        assert!(!native1.has_same_public_key(&p256_1));
    }

    #[test]
    fn test_wrapper_serialized_size_calculation() {
        // V1 wrapper
        let v1_wrapper = SmartAccountSignerWrapper::V1(vec![
            LegacySmartAccountSigner {
                key: Pubkey::new_unique(),
                permissions: Permissions::all(),
            },
            LegacySmartAccountSigner {
                key: Pubkey::new_unique(),
                permissions: Permissions { mask: 0b011 },
            },
        ]);

        let calculated_size = v1_wrapper.serialized_size();
        let mut buf = Vec::new();
        v1_wrapper.serialize(&mut buf).unwrap();
        assert_eq!(calculated_size, buf.len());

        // V2 wrapper with mixed signers
        let v2_wrapper = SmartAccountSignerWrapper::V2(vec![
            SmartAccountSigner::Native {
                key: Pubkey::new_unique(),
                permissions: Permissions::all(),
            },
            SmartAccountSigner::Ed25519External {
                key_id: Pubkey::new_unique(),
                permissions: Permissions::all(),
                data: Ed25519ExternalData {
                    external_pubkey: [0xAB; 32],
                    session_key_data: SessionKeyData::default(),
                },
            },
        ]);

        let calculated_size = v2_wrapper.serialized_size();
        let mut buf = Vec::new();
        v2_wrapper.serialize(&mut buf).unwrap();
        assert_eq!(calculated_size, buf.len());
    }

    #[test]
    fn test_client_data_json_reconstruction_params() {
        // Test TYPE_CREATE
        let create_params = ClientDataJsonReconstructionParams::new(
            true, false, false, false, None
        );
        assert!(create_params.is_create());
        assert!(!create_params.is_cross_origin());
        assert!(!create_params.is_http());
        assert!(!create_params.has_google_extra());
        assert_eq!(create_params.get_port(), None);

        // Test TYPE_GET with flags and port
        let get_params = ClientDataJsonReconstructionParams::new(
            false, true, true, true, Some(8080)
        );
        assert!(!get_params.is_create());
        assert!(get_params.is_cross_origin());
        assert!(get_params.is_http());
        assert!(get_params.has_google_extra());
        assert_eq!(get_params.get_port(), Some(8080));
    }

    #[test]
    fn test_wrapper_all_permissions_valid() {
        // Valid V1 wrapper
        let valid_v1 = SmartAccountSignerWrapper::V1(vec![
            LegacySmartAccountSigner {
                key: Pubkey::new_unique(),
                permissions: Permissions { mask: 7 }, // 0b111 - valid
            },
        ]);
        assert!(valid_v1.all_permissions_valid());

        // Invalid V1 wrapper
        let invalid_v1 = SmartAccountSignerWrapper::V1(vec![
            LegacySmartAccountSigner {
                key: Pubkey::new_unique(),
                permissions: Permissions { mask: 8 }, // >= 8 - invalid
            },
        ]);
        assert!(!invalid_v1.all_permissions_valid());

        // Valid V2 wrapper
        let valid_v2 = SmartAccountSignerWrapper::V2(vec![
            SmartAccountSigner::Native {
                key: Pubkey::new_unique(),
                permissions: Permissions { mask: 0 }, // valid
            },
        ]);
        assert!(valid_v2.all_permissions_valid());

        // Invalid V2 wrapper
        let invalid_v2 = SmartAccountSignerWrapper::V2(vec![
            SmartAccountSigner::Native {
                key: Pubkey::new_unique(),
                permissions: Permissions { mask: 15 }, // >= 8 - invalid
            },
        ]);
        assert!(!invalid_v2.all_permissions_valid());
    }

    #[test]
    fn test_wrapper_counter_updates() {
        let key_id = Pubkey::new_unique();
        let mut wrapper = SmartAccountSignerWrapper::V2(vec![
            SmartAccountSigner::P256Webauthn {
                key_id,
                permissions: Permissions::all(),
                data: P256WebauthnData {
                    compressed_pubkey: [0x02; 33],
                    rp_id_len: 8,
                    rp_id: [0x00; 32],
                    rp_id_hash: [0xAB; 32],
                    counter: 100,
                    session_key_data: SessionKeyData::default(),
                },
            },
        ]);

        // Update counter
        let updates = vec![(key_id, 101)];
        assert!(wrapper.apply_counter_updates(&updates).is_ok());

        // Verify counter was updated
        if let Some(SmartAccountSigner::P256Webauthn { data, .. }) = wrapper.find_mut(&key_id) {
            assert_eq!(data.counter, 101);
        } else {
            panic!("Expected P256Webauthn signer");
        }

        // V1 wrapper should fail counter updates
        let mut v1_wrapper = SmartAccountSignerWrapper::V1(vec![
            LegacySmartAccountSigner {
                key: Pubkey::new_unique(),
                permissions: Permissions::all(),
            },
        ]);
        assert!(v1_wrapper.apply_counter_updates(&updates).is_err());
    }
}

