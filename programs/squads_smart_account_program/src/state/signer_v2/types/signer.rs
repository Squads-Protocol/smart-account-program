use anchor_lang::prelude::*;
use super::{Ed25519ExternalData, ExternalSignerData, P256NativeData, P256WebauthnData, Secp256k1Data, SignerType};
use crate::state::signer_v2::{LegacySmartAccountSigner, Permissions};

// ============================================================================
// Unified V2 Signer Enum
// ============================================================================

/// Unified V2 signer enum
/// Each variant contains:
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
        permissions: Permissions,
        data: P256WebauthnData,
        nonce: u64,
    },

    /// Secp256k1/Ethereum-style signer (verified via secp256k1 precompile introspection)
    Secp256k1 {
        permissions: Permissions,
        data: Secp256k1Data,
        nonce: u64,
    },

    /// Ed25519 external signer (verified via ed25519 precompile introspection, NOT native Signer)
    Ed25519External {
        permissions: Permissions,
        data: Ed25519ExternalData,
        nonce: u64,
    },

    /// P256 native signer (verified via secp256r1 precompile, raw message hash — no WebAuthn wrapping)
    P256Native {
        permissions: Permissions,
        data: P256NativeData,
        nonce: u64,
    },
}

impl SmartAccountSigner {
    /// Get the key (Native) or truncated external key for this signer
    pub fn key(&self) -> Pubkey {
        match self {
            Self::Native { key, .. } => *key,
            Self::P256Webauthn { data, .. } => {
                Pubkey::new_from_array(data.compressed_pubkey[..32].try_into().unwrap())
            }
            Self::Secp256k1 { data, .. } => {
                Pubkey::new_from_array(data.uncompressed_pubkey[..32].try_into().unwrap())
            }
            Self::Ed25519External { data, .. } => {
                Pubkey::new_from_array(data.external_pubkey)
            }
            Self::P256Native { data, .. } => {
                Pubkey::new_from_array(data.compressed_pubkey[..32].try_into().unwrap())
            }
        }
    }

    /// Get permissions for this signer
    pub fn permissions(&self) -> Permissions {
        match self {
            Self::Native { permissions, .. }
            | Self::P256Webauthn { permissions, .. }
            | Self::Secp256k1 { permissions, .. }
            | Self::Ed25519External { permissions, .. }
            | Self::P256Native { permissions, .. } => *permissions,
        }
    }

    /// Get signer type discriminator
    pub fn signer_type(&self) -> SignerType {
        match self {
            Self::Native { .. } => SignerType::Native,
            Self::P256Webauthn { .. } => SignerType::P256Webauthn,
            Self::Secp256k1 { .. } => SignerType::Secp256k1,
            Self::Ed25519External { .. } => SignerType::Ed25519External,
            Self::P256Native { .. } => SignerType::P256Native,
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
            Self::P256Native { data, .. } => Some(&data.compressed_pubkey),
        }
    }

    /// Check if two signers have the same underlying public key
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
            (Self::P256Native { data: d1, .. }, Self::P256Native { data: d2, .. }) => {
                d1.compressed_pubkey == d2.compressed_pubkey
            }
            // P256Webauthn and P256Native share the same curve — compare compressed pubkeys
            (Self::P256Webauthn { data: d1, .. }, Self::P256Native { data: d2, .. })
            | (Self::P256Native { data: d2, .. }, Self::P256Webauthn { data: d1, .. }) => {
                d1.compressed_pubkey == d2.compressed_pubkey
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
            Self::P256Native { data, .. } => {
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
            Self::P256Native { data, .. } => data.has_active_session_key(current_timestamp),
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
            Self::P256Native { data, .. } => {
                data.session_key_data.key == *pubkey && data.has_active_session_key(current_timestamp)
            }
        }
    }

    /// Get session key data if the given pubkey matches this signer's session key
    /// Returns None for native signers or if the pubkey doesn't match
    /// Note: Does NOT check expiration - caller should validate expiration separately
    pub fn get_session_key_data_if_matches(&self, pubkey: &Pubkey) -> Option<super::SessionKeyData> {
        if *pubkey == Pubkey::default() {
            return None;
        }
        match self {
            Self::Native { .. } => None,
            Self::P256Webauthn { data, .. } => {
                if data.session_key_data.key == *pubkey {
                    Some(data.session_key_data.clone())
                } else {
                    None
                }
            }
            Self::Secp256k1 { data, .. } => {
                if data.session_key_data.key == *pubkey {
                    Some(data.session_key_data.clone())
                } else {
                    None
                }
            }
            Self::Ed25519External { data, .. } => {
                if data.session_key_data.key == *pubkey {
                    Some(data.session_key_data.clone())
                } else {
                    None
                }
            }
            Self::P256Native { data, .. } => {
                if data.session_key_data.key == *pubkey {
                    Some(data.session_key_data.clone())
                } else {
                    None
                }
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
            Self::P256Native { data, .. } => data.set_session_key(key, expiration, current_timestamp),
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
            Self::P256Native { data, .. } => {
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

    /// Get nonce (only for external signers)
    pub fn nonce(&self) -> Option<u64> {
        match self {
            Self::P256Webauthn { nonce, .. }
            | Self::Secp256k1 { nonce, .. }
            | Self::Ed25519External { nonce, .. }
            | Self::P256Native { nonce, .. } => Some(*nonce),
            _ => None,
        }
    }

    /// Set nonce (only for external signers)
    pub fn set_nonce(&mut self, new_nonce: u64) -> Result<()> {
        match self {
            Self::P256Webauthn { nonce, .. }
            | Self::Secp256k1 { nonce, .. }
            | Self::Ed25519External { nonce, .. }
            | Self::P256Native { nonce, .. } => {
                *nonce = new_nonce;
                Ok(())
            }
            _ => Err(error!(crate::errors::SmartAccountError::InvalidSignerType)),
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
}
