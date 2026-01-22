use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};
use std::io::{Read, Write};

// Re-export from settings for convenience
pub use super::settings::{Permission, Permissions, SmartAccountSigner};

/// V2 signer type discriminator (explicit u8 values for stability)
#[derive(Clone, Copy, PartialEq, Eq, Debug, BorshSerialize, BorshDeserialize)]
#[repr(u8)]
pub enum SignerTypeV2 {
    Native = 0,
    P256Webauthn = 1,
    Secp256k1 = 2,
    Ed25519External = 3,
}

/// P256/WebAuthn signer data (73 bytes)
/// - compressed_pubkey: 33 bytes (compressed P256 public key)
/// - rp_id_hash: 32 bytes (SHA256 of Relying Party ID for validation)
/// - counter: 8 bytes (WebAuthn counter for replay protection)
#[derive(Clone, BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq)]
pub struct P256WebauthnDataV2 {
    pub compressed_pubkey: [u8; 33],
    pub rp_id_hash: [u8; 32],
    pub counter: u64,
}

impl P256WebauthnDataV2 {
    pub const SIZE: usize = 33 + 32 + 8; // 73 bytes
}

/// Secp256k1 signer data (85 bytes)
/// - uncompressed_pubkey: 64 bytes (uncompressed secp256k1 public key, no 0x04 prefix)
/// - eth_address: 20 bytes (keccak256(pubkey)[12..32])
/// - has_eth_address: 1 byte (whether eth_address is populated/validated)
#[derive(Clone, BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq)]
pub struct Secp256k1DataV2 {
    pub uncompressed_pubkey: [u8; 64],
    pub eth_address: [u8; 20],
    pub has_eth_address: bool,
}

impl Secp256k1DataV2 {
    pub const SIZE: usize = 64 + 20 + 1; // 85 bytes
}

/// Ed25519 external signer data (32 bytes)
/// - external_pubkey: 32 bytes (Ed25519 public key verified via precompile, not native Signer)
#[derive(Clone, BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq)]
pub struct Ed25519ExternalDataV2 {
    pub external_pubkey: [u8; 32],
}

impl Ed25519ExternalDataV2 {
    pub const SIZE: usize = 32;
}

/// Unified V2 signer enum
/// Each variant contains:
/// - key_id: Pubkey (deterministically derived from signer type + public key)
/// - permissions: Permissions (same bitmask as V1)
/// - type-specific data
#[derive(Clone, BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq)]
pub enum SmartAccountSignerV2 {
    /// Native Solana Ed25519 signer (verified via AccountInfo.is_signer)
    Native {
        key: Pubkey,
        permissions: Permissions,
    },

    /// P256/WebAuthn passkey signer (verified via secp256r1 precompile introspection)
    P256Webauthn {
        key_id: Pubkey,
        permissions: Permissions,
        data: P256WebauthnDataV2,
    },

    /// Secp256k1/Ethereum-style signer (verified via secp256k1 precompile introspection)
    Secp256k1 {
        key_id: Pubkey,
        permissions: Permissions,
        data: Secp256k1DataV2,
    },

    /// Ed25519 external signer (verified via ed25519 precompile introspection, NOT native Signer)
    Ed25519External {
        key_id: Pubkey,
        permissions: Permissions,
        data: Ed25519ExternalDataV2,
    },
}

impl SmartAccountSignerV2 {
    /// Derive deterministic key_id from signer type and canonical public key bytes
    pub fn derive_key_id(signer_type: SignerTypeV2, canonical_key: &[u8]) -> Pubkey {
        use anchor_lang::solana_program::hash::hash;

        let mut data = Vec::with_capacity(1 + canonical_key.len());
        data.push(signer_type as u8);
        data.extend_from_slice(canonical_key);

        Pubkey::new_from_array(hash(&data).to_bytes())
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
    pub fn signer_type(&self) -> SignerTypeV2 {
        match self {
            Self::Native { .. } => SignerTypeV2::Native,
            Self::P256Webauthn { .. } => SignerTypeV2::P256Webauthn,
            Self::Secp256k1 { .. } => SignerTypeV2::Secp256k1,
            Self::Ed25519External { .. } => SignerTypeV2::Ed25519External,
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

    /// Convert from V1 SmartAccountSigner (always Native)
    pub fn from_v1(signer: &SmartAccountSigner) -> Self {
        Self::Native {
            key: signer.key,
            permissions: signer.permissions,
        }
    }

    /// Convert to V1 SmartAccountSigner (only if Native)
    pub fn to_v1(&self) -> Option<SmartAccountSigner> {
        match self {
            Self::Native { key, permissions } => Some(SmartAccountSigner {
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
            Self::P256Webauthn { .. } => 33 + P256WebauthnDataV2::SIZE, // 32 + 1 + 73 = 106
            Self::Secp256k1 { .. } => 33 + Secp256k1DataV2::SIZE,       // 32 + 1 + 85 = 118
            Self::Ed25519External { .. } => 33 + Ed25519ExternalDataV2::SIZE, // 32 + 1 + 32 = 65
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
                (SignerTypeV2::Native as u8, payload)
            }
            Self::P256Webauthn {
                key_id,
                permissions,
                data,
            } => {
                let mut payload = Vec::with_capacity(106);
                payload.extend_from_slice(key_id.as_ref());
                payload.push(permissions.mask);
                payload.extend_from_slice(&data.compressed_pubkey);
                payload.extend_from_slice(&data.rp_id_hash);
                payload.extend_from_slice(&data.counter.to_le_bytes());
                (SignerTypeV2::P256Webauthn as u8, payload)
            }
            Self::Secp256k1 {
                key_id,
                permissions,
                data,
            } => {
                let mut payload = Vec::with_capacity(118);
                payload.extend_from_slice(key_id.as_ref());
                payload.push(permissions.mask);
                payload.extend_from_slice(&data.uncompressed_pubkey);
                payload.extend_from_slice(&data.eth_address);
                payload.push(data.has_eth_address as u8);
                (SignerTypeV2::Secp256k1 as u8, payload)
            }
            Self::Ed25519External {
                key_id,
                permissions,
                data,
            } => {
                let mut payload = Vec::with_capacity(65);
                payload.extend_from_slice(key_id.as_ref());
                payload.push(permissions.mask);
                payload.extend_from_slice(&data.external_pubkey);
                (SignerTypeV2::Ed25519External as u8, payload)
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

    /// Parse from packed P256Webauthn payload (106 bytes)
    pub fn from_packed_p256(payload: &[u8]) -> Result<Self> {
        if payload.len() != 106 {
            return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
        }
        let key_id = Pubkey::try_from(&payload[..32])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;
        let permissions = Permissions { mask: payload[32] };

        let mut compressed_pubkey = [0u8; 33];
        compressed_pubkey.copy_from_slice(&payload[33..66]);

        let mut rp_id_hash = [0u8; 32];
        rp_id_hash.copy_from_slice(&payload[66..98]);

        let counter = u64::from_le_bytes(
            payload[98..106]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        Ok(Self::P256Webauthn {
            key_id,
            permissions,
            data: P256WebauthnDataV2 {
                compressed_pubkey,
                rp_id_hash,
                counter,
            },
        })
    }

    /// Parse from packed Secp256k1 payload (118 bytes)
    pub fn from_packed_secp256k1(payload: &[u8]) -> Result<Self> {
        if payload.len() != 118 {
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

        Ok(Self::Secp256k1 {
            key_id,
            permissions,
            data: Secp256k1DataV2 {
                uncompressed_pubkey,
                eth_address,
                has_eth_address,
            },
        })
    }

    /// Parse from packed Ed25519External payload (65 bytes)
    pub fn from_packed_ed25519_external(payload: &[u8]) -> Result<Self> {
        if payload.len() != 65 {
            return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
        }
        let key_id = Pubkey::try_from(&payload[..32])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;
        let permissions = Permissions { mask: payload[32] };

        let mut external_pubkey = [0u8; 32];
        external_pubkey.copy_from_slice(&payload[33..65]);

        Ok(Self::Ed25519External {
            key_id,
            permissions,
            data: Ed25519ExternalDataV2 { external_pubkey },
        })
    }
}

/// Version discriminator values (stored in byte 3 of Vec length)
pub const SIGNERS_VERSION_V1: u8 = 0x00;
pub const SIGNERS_VERSION_V2: u8 = 0x01;

/// Maximum signers to prevent overflow into version byte
pub const MAX_SIGNERS: usize = 65535;

/// Packed V2 entry header: <u8 tag><u16 payload_len LE><u8 flags>
pub const ENTRY_HEADER_LEN: usize = 4;

/// Wrapper for signers that supports both V1 and V2 formats
/// Uses custom Borsh serialization to maintain V1 backward compatibility
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SmartAccountSignerWrapper {
    V1(Vec<SmartAccountSigner>),
    V2(Vec<SmartAccountSignerV2>),
}

impl Default for SmartAccountSignerWrapper {
    fn default() -> Self {
        Self::V1(Vec::new())
    }
}

impl SmartAccountSignerWrapper {
    /// Create a V1 wrapper from a Vec of SmartAccountSigner
    pub fn from_v1_signers(signers: Vec<SmartAccountSigner>) -> Self {
        Self::V1(signers)
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
    /// 
    /// # Panics
    /// Panics if called on a V2 wrapper. Use `try_as_v1_slice()` for a safe version.
    pub fn as_v1_slice(&self) -> &[SmartAccountSigner] {
        match self {
            Self::V1(signers) => signers.as_slice(),
            Self::V2(_) => panic!("Cannot get V1 slice from V2 wrapper - use as_v2() instead"),
        }
    }

    /// Safely try to get a reference to the inner V1 signers slice.
    /// Returns None for V2 wrappers.
    pub fn try_as_v1_slice(&self) -> Option<&[SmartAccountSigner]> {
        match self {
            Self::V1(signers) => Some(signers.as_slice()),
            Self::V2(_) => None,
        }
    }

    /// Get signers as V2 format (canonical view)
    pub fn as_v2(&self) -> Vec<SmartAccountSignerV2> {
        match self {
            Self::V1(signers) => signers.iter().map(SmartAccountSignerV2::from_v1).collect(),
            Self::V2(signers) => signers.clone(),
        }
    }

    /// Get signers as V1 format (only if all are Native)
    pub fn as_v1(&self) -> Option<Vec<SmartAccountSigner>> {
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
    pub fn find(&self, key: &Pubkey) -> Option<SmartAccountSignerV2> {
        match self {
            Self::V1(signers) => signers
                .iter()
                .find(|s| &s.key == key)
                .map(SmartAccountSignerV2::from_v1),
            Self::V2(signers) => signers.iter().find(|s| &s.key() == key).cloned(),
        }
    }

    /// Find index of a signer by key/key_id
    pub fn find_index(&self, key: &Pubkey) -> Option<usize> {
        match self {
            Self::V1(signers) => signers.iter().position(|s| &s.key == key),
            Self::V2(signers) => signers.iter().position(|s| &s.key() == key),
        }
    }

    /// Get signer at index
    pub fn get(&self, index: usize) -> Option<SmartAccountSignerV2> {
        match self {
            Self::V1(signers) => signers.get(index).map(SmartAccountSignerV2::from_v1),
            Self::V2(signers) => signers.get(index).cloned(),
        }
    }

    /// Push a V2 signer (converts to V2 format if needed)
    pub fn push_v2(&mut self, signer: SmartAccountSignerV2) {
        match self {
            Self::V1(signers) => {
                // If signer is Native, can stay V1
                if let Some(v1_signer) = signer.to_v1() {
                    signers.push(v1_signer);
                } else {
                    // Convert to V2
                    let mut v2_signers: Vec<SmartAccountSignerV2> =
                        signers.iter().map(SmartAccountSignerV2::from_v1).collect();
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
    pub fn remove(&mut self, index: usize) -> SmartAccountSignerV2 {
        match self {
            Self::V1(signers) => SmartAccountSignerV2::from_v1(&signers.remove(index)),
            Self::V2(signers) => signers.remove(index),
        }
    }

    /// Sort signers by key
    pub fn sort_by_key<F, K>(&mut self, mut f: F)
    where
        F: FnMut(&SmartAccountSignerV2) -> K,
        K: Ord,
    {
        match self {
            Self::V1(signers) => {
                signers.sort_by_key(|s| f(&SmartAccountSignerV2::from_v1(s)));
            }
            Self::V2(signers) => {
                signers.sort_by_key(|s| f(s));
            }
        }
    }

    /// Force V2 format (for when external signers are added)
    pub fn force_v2(&mut self) {
        if let Self::V1(signers) = self {
            let v2_signers: Vec<SmartAccountSignerV2> =
                signers.iter().map(SmartAccountSignerV2::from_v1).collect();
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
    pub fn last(&self) -> Option<SmartAccountSignerV2> {
        match self {
            Self::V1(signers) => signers.last().map(SmartAccountSignerV2::from_v1),
            Self::V2(signers) => signers.last().cloned(),
        }
    }

    /// Iterate over signers as V2
    pub fn iter_v2(&self) -> impl Iterator<Item = SmartAccountSignerV2> + '_ {
        let signers = self.as_v2();
        signers.into_iter()
    }

    /// Calculate the serialized size in bytes
    pub fn serialized_size(&self) -> usize {
        match self {
            Self::V1(signers) => {
                // 4 bytes for length + 33 bytes per V1 signer
                4 + signers.len() * SmartAccountSigner::INIT_SPACE
            }
            Self::V2(signers) => {
                // 4 bytes for length + variable per signer (header + payload)
                4 + signers.iter().map(|s| {
                    let (_, payload) = s.to_packed_payload();
                    ENTRY_HEADER_LEN + payload.len()
                }).sum::<usize>()
            }
        }
    }

    /// Add a V1 signer (for backward compatibility)
    pub fn add_signer(&mut self, signer: SmartAccountSigner) {
        self.push_v2(SmartAccountSignerV2::from_v1(&signer));
    }

    /// Remove signer by key and return it
    pub fn remove_signer(&mut self, key: &Pubkey) -> Option<SmartAccountSignerV2> {
        let index = self.find_index(key)?;
        Some(self.remove(index))
    }

    /// Sort signers by key (V2 key)
    pub fn sort_by_signer_key(&mut self) {
        self.sort_by_key(|s| s.key());
    }

    /// Check for duplicate signers
    pub fn has_duplicates(&self) -> bool {
        let signers = self.as_v2();
        signers.windows(2).any(|win| win[0].key() == win[1].key())
    }

    /// Check if all signers have valid permissions (mask < 8)
    pub fn all_permissions_valid(&self) -> bool {
        match self {
            Self::V1(signers) => signers.iter().all(|s| s.permissions.mask < 8),
            Self::V2(signers) => signers.iter().all(|s| s.permissions().mask < 8),
        }
    }
}

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
                    let signer = SmartAccountSigner::deserialize_reader(reader)?;
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
                        x if x == SignerTypeV2::Native as u8 => {
                            SmartAccountSignerV2::from_packed_native(&payload)
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?
                        }
                        x if x == SignerTypeV2::P256Webauthn as u8 => {
                            SmartAccountSignerV2::from_packed_p256(&payload)
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?
                        }
                        x if x == SignerTypeV2::Secp256k1 as u8 => {
                            SmartAccountSignerV2::from_packed_secp256k1(&payload)
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?
                        }
                        x if x == SignerTypeV2::Ed25519External as u8 => {
                            SmartAccountSignerV2::from_packed_ed25519_external(&payload)
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
            SmartAccountSigner {
                key: Pubkey::new_unique(),
                permissions: Permissions::all(),
            },
            SmartAccountSigner {
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
            SmartAccountSignerV2::Native {
                key: Pubkey::new_unique(),
                permissions: Permissions::all(),
            },
            SmartAccountSignerV2::P256Webauthn {
                key_id: Pubkey::new_unique(),
                permissions: Permissions { mask: 0b111 },
                data: P256WebauthnDataV2 {
                    compressed_pubkey: [0x02; 33],
                    rp_id_hash: [0xAB; 32],
                    counter: 42,
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
        let key_id = SmartAccountSignerV2::derive_key_id(SignerTypeV2::P256Webauthn, &pubkey_bytes);

        // Same input should produce same key_id
        let key_id_2 = SmartAccountSignerV2::derive_key_id(SignerTypeV2::P256Webauthn, &pubkey_bytes);
        assert_eq!(key_id, key_id_2);

        // Different type should produce different key_id
        let key_id_3 = SmartAccountSignerV2::derive_key_id(SignerTypeV2::Secp256k1, &pubkey_bytes);
        assert_ne!(key_id, key_id_3);
    }

    #[test]
    fn test_wrapper_force_v2() {
        let signers = vec![SmartAccountSigner {
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
        let mut wrapper = SmartAccountSignerWrapper::V1(vec![SmartAccountSigner {
            key: Pubkey::new_unique(),
            permissions: Permissions::all(),
        }]);

        assert_eq!(wrapper.version(), SIGNERS_VERSION_V1);

        // Push an external signer
        wrapper.push_v2(SmartAccountSignerV2::P256Webauthn {
            key_id: Pubkey::new_unique(),
            permissions: Permissions::all(),
            data: P256WebauthnDataV2 {
                compressed_pubkey: [0x02; 33],
                rp_id_hash: [0xAB; 32],
                counter: 0,
            },
        });

        assert_eq!(wrapper.version(), SIGNERS_VERSION_V2);
        assert_eq!(wrapper.len(), 2);
    }
}

