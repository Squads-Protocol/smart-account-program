use anchor_lang::prelude::*;
use super::{Ed25519ExternalData, P256NativeData, P256WebauthnData, Secp256k1Data, SessionKeyData, SignerType};
use super::signer::SmartAccountSigner;
use crate::state::signer_v2::Permissions;

/// Raw instruction data parsing for SmartAccountSigner.
///
/// Converts untrusted instruction data into validated SmartAccountSigner instances.
/// Used by settings instructions (AddSigner) to create new signers from user input.
impl SmartAccountSigner {
    /// Create a SmartAccountSigner from raw instruction data.
    ///
    /// # Arguments
    /// - `signer_type`: The type of signer (0=Native, 1=P256Webauthn, 2=Secp256k1, 3=Ed25519External)
    /// - `key`: For Native signers, the signer's pubkey. Ignored for external signers.
    /// - `permissions`: The permissions for this signer
    /// - `signer_data`: Signer-specific data:
    ///   - Native: empty (0 bytes)
    ///   - P256Webauthn: 74 bytes (compressed_pubkey(33) + rp_id_len(1) + rp_id(32) + counter(8))
    ///     Note: rp_id_hash is derived from rp_id, not provided by caller
    ///   - Secp256k1: 64 bytes (uncompressed_pubkey(64)); eth_address is derived on-chain
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
            4 => SignerType::P256Native,
            _ => return Err(error!(crate::errors::SmartAccountError::InvalidSignerType)),
        };

        match signer_type {
            SignerType::Native => {
                if key == Pubkey::default() {
                    return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
                }
                if !signer_data.is_empty() {
                    return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
                }
                Ok(Self::Native { key, permissions })
            }
            SignerType::P256Webauthn => {
                // Layout: compressed_pubkey(33) + rp_id_len(1) + rp_id(32) + counter(8) = 74 bytes
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
                use anchor_lang::solana_program::hash::hash;
                let rp_id_hash_result = hash(&rp_id[..rp_id_len as usize]);
                let mut rp_id_hash = [0u8; 32];
                rp_id_hash.copy_from_slice(&rp_id_hash_result.to_bytes());

                let counter = u64::from_le_bytes(
                    signer_data[66..74]
                        .try_into()
                        .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
                );

                Ok(Self::P256Webauthn {
                    permissions,
                    data: P256WebauthnData {
                        compressed_pubkey,
                        rp_id_len,
                        rp_id,
                        rp_id_hash,
                        counter,
                        session_key_data: SessionKeyData::default(),
                    },
                    nonce: 0,
                })
            }
            SignerType::Secp256k1 => {
                // Layout: uncompressed_pubkey(64) = 64 bytes
                // eth_address is derived on-chain from uncompressed_pubkey (don't trust user input)
                if signer_data.len() != 64 {
                    return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
                }
                let mut uncompressed_pubkey = [0u8; 64];
                uncompressed_pubkey.copy_from_slice(&signer_data[0..64]);

                // Derive eth_address on-chain: keccak256(pubkey)[12..32]
                let eth_address = crate::state::signer_v2::secp256k1_syscall::compute_eth_address(&uncompressed_pubkey);

                Ok(Self::Secp256k1 {
                    permissions,
                    data: Secp256k1Data {
                        uncompressed_pubkey,
                        eth_address,
                        has_eth_address: true,
                        session_key_data: SessionKeyData::default(),
                    },
                    nonce: 0,
                })
            }
            SignerType::Ed25519External => {
                // Layout: external_pubkey(32) = 32 bytes
                if signer_data.len() != 32 {
                    return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
                }
                let mut external_pubkey = [0u8; 32];
                external_pubkey.copy_from_slice(&signer_data[0..32]);

                Ok(Self::Ed25519External {
                    permissions,
                    data: Ed25519ExternalData {
                        external_pubkey,
                        session_key_data: SessionKeyData::default(),
                    },
                    nonce: 0,
                })
            }
            SignerType::P256Native => {
                // Layout: compressed_pubkey(33) = 33 bytes
                if signer_data.len() != 33 {
                    return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
                }
                let mut compressed_pubkey = [0u8; 33];
                compressed_pubkey.copy_from_slice(&signer_data[0..33]);

                Ok(Self::P256Native {
                    permissions,
                    data: P256NativeData {
                        compressed_pubkey,
                        session_key_data: SessionKeyData::default(),
                    },
                    nonce: 0,
                })
            }
        }
    }
}
