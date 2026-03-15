use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};
use std::io::{Read, Write};
use super::{LegacySmartAccountSigner, Permission, SignerType, SmartAccountSigner, MAX_SIGNERS};

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

    /// Find a signer by key (native or truncated external)
    pub fn find(&self, key: &Pubkey) -> Option<SmartAccountSigner> {
        match self {
            Self::V1(signers) => signers
                .iter()
                .find(|s| &s.key == key)
                .map(SmartAccountSigner::from_v1),
            Self::V2(signers) => signers.iter().find(|s| &s.key() == key).cloned(),
        }
    }

    /// Find a mutable reference to a signer by key (V2 only).
    /// Returns None if the wrapper is V1 (since V1 signers don't have counters).
    pub fn find_mut(&mut self, key: &Pubkey) -> Option<&mut SmartAccountSigner> {
        match self {
            Self::V1(_) => None, // V1 signers don't have counters
            Self::V2(signers) => signers.iter_mut().find(|s| &s.key() == key),
        }
    }

    /// Update the WebAuthn counter for a signer by key.
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

    /// Update nonce for a signer by key.
    pub fn update_signer_nonce(&mut self, key_id: &Pubkey, new_nonce: u64) -> Result<()> {
        match self {
            Self::V1(_) => Err(error!(crate::errors::SmartAccountError::InvalidSignerType)),
            Self::V2(signers) => {
                let signer = signers
                    .iter_mut()
                    .find(|s| &s.key() == key_id)
                    .ok_or_else(|| error!(crate::errors::SmartAccountError::NotASigner))?;
                signer.set_nonce(new_nonce)
            }
        }
    }

    /// Find index of a signer by key
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

    /// Get a single signer from the wrapper.
    /// Returns an error if the wrapper doesn't contain exactly one signer.
    /// This is useful for operations like AddSigner that expect a single signer.
    pub fn single(&self) -> Result<SmartAccountSigner> {
        require_eq!(self.len(), 1, crate::errors::SmartAccountError::InvalidInstructionArgs);
        self.get(0).ok_or(crate::errors::SmartAccountError::InvalidInstructionArgs.into())
    }

    /// Get the key of a single signer in the wrapper.
    /// Returns an error if the wrapper doesn't contain exactly one signer.
    /// This is useful for operations like AddSigner that expect a single signer.
    pub fn single_key(&self) -> Result<Pubkey> {
        Ok(self.single()?.key())
    }

    /// Add a signer, strictly preserving the wrapper format.
    /// - If wrapper is V1 and signer is Native: stays V1
    /// - If wrapper is V1 and signer is external: ERROR - must call force_v2() first
    /// - If wrapper is V2: stays V2
    ///
    /// To add external signers to V1 wrapper, explicitly migrate first using force_v2().
    fn push(&mut self, signer: SmartAccountSigner) -> Result<()> {
        match self {
            Self::V1(signers) => {
                // If signer is Native, can stay V1
                if let Some(v1_signer) = signer.to_v1() {
                    signers.push(v1_signer);
                    Ok(())
                } else {
                    // Reject external signers in V1 - require explicit migration
                    err!(crate::errors::SmartAccountError::SignerTypeMismatch)
                }
            }
            Self::V2(signers) => {
                signers.push(signer);
                Ok(())
            }
        }
    }

    /// Remove signer at index.
    /// Callers must ensure `index < self.len()` (e.g. via `find_index`).
    pub(crate) fn remove(&mut self, index: usize) -> SmartAccountSigner {
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

    /// Add a signer, strictly preserving format.
    /// Returns error if trying to add external signer to V1 wrapper - must migrate first.
    /// Checks for both truncated key collision and full public key collision.
    pub fn add_signer(&mut self, signer: SmartAccountSigner) -> Result<()> {
        require!(
            !self.has_duplicate_truncated_key(&signer),
            crate::errors::SmartAccountError::DuplicateSigner
        );
        require!(
            !self.has_duplicate_public_key(&signer),
            crate::errors::SmartAccountError::DuplicateSigner
        );
        self.push(signer)
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

    /// Check for duplicate signers by key
    pub fn has_duplicates(&self) -> bool {
        let mut seen: std::collections::BTreeSet<Pubkey> = std::collections::BTreeSet::new();
        self.iter_v2().any(|s| !seen.insert(s.key()))
    }

    /// Check if a new signer would collide on the truncated key.
    pub fn has_duplicate_truncated_key(&self, new_signer: &SmartAccountSigner) -> bool {
        self.find(&new_signer.key()).is_some()
    }

    /// Check if any existing signer has the same public key as the given signer.
    /// This prevents adding the same external public key with a different truncated key.
    pub fn has_duplicate_public_key(&self, new_signer: &SmartAccountSigner) -> bool {
        let mut seen: std::collections::BTreeSet<Vec<u8>> = std::collections::BTreeSet::new();
        for existing in self.iter_v2() {
            let key_bytes: Vec<u8> = match existing {
                SmartAccountSigner::Native { key, .. } => key.to_bytes().to_vec(),
                SmartAccountSigner::P256Webauthn { data, .. } => data.compressed_pubkey.to_vec(),
                SmartAccountSigner::Secp256k1 { data, .. } => data.uncompressed_pubkey.to_vec(),
                SmartAccountSigner::Ed25519External { data, .. } => data.external_pubkey.to_vec(),
                SmartAccountSigner::P256Native { data, .. } => data.compressed_pubkey.to_vec(),
            };
            seen.insert(key_bytes);
        }

        let new_key_bytes: Vec<u8> = match new_signer {
            SmartAccountSigner::Native { key, .. } => key.to_bytes().to_vec(),
            SmartAccountSigner::P256Webauthn { data, .. } => data.compressed_pubkey.to_vec(),
            SmartAccountSigner::Secp256k1 { data, .. } => data.uncompressed_pubkey.to_vec(),
            SmartAccountSigner::Ed25519External { data, .. } => data.external_pubkey.to_vec(),
            SmartAccountSigner::P256Native { data, .. } => data.compressed_pubkey.to_vec(),
        };

        seen.contains(&new_key_bytes)
    }

    /// Find an external signer by their active session key.
    /// Returns the signer if the pubkey matches an active session key.
    pub fn find_by_session_key(&self, pubkey: &Pubkey, current_timestamp: u64) -> Option<SmartAccountSigner> {
        self.iter_v2()
            .find(|s| s.is_valid_session_key(pubkey, current_timestamp))
    }

    /// Check if any signer already has this pubkey registered as a session key.
    /// Unlike `find_by_session_key`, this does NOT check expiration — it catches
    /// any existing session key assignment regardless of whether it's active.
    pub fn has_session_key_assigned(&self, pubkey: &Pubkey) -> bool {
        if *pubkey == Pubkey::default() {
            return false;
        }
        self.iter_v2().any(|s| s.get_session_key() == Some(*pubkey))
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
                        x if x == SignerType::P256Native as u8 => {
                            SmartAccountSigner::from_packed_p256_native(&payload)
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
