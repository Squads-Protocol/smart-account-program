use anchor_lang::prelude::*;
use super::{Ed25519ExternalData, P256NativeData, P256WebauthnData, Secp256k1Data, SessionKeyData, SignerType};
use super::signer::SmartAccountSigner;
use crate::state::signer_v2::Permissions;

/// Packed serialization methods for SmartAccountSigner.
///
/// These handle the V2 on-chain binary encoding used by SmartAccountSignerWrapper's
/// custom Borsh serialization. Each signer is encoded as:
///   <u8 tag><u16 payload_len LE><u8 flags><payload>
impl SmartAccountSigner {
    /// Payload size for packed encoding (without the 4-byte header)
    pub fn packed_payload_size(&self) -> usize {
        match self {
            Self::Native { .. } => 33,                              // 32 (key) + 1 (permissions)
            Self::P256Webauthn { .. } => P256WebauthnData::PACKED_PAYLOAD_LEN,
            Self::Secp256k1 { .. } => Secp256k1Data::PACKED_PAYLOAD_LEN,
            Self::Ed25519External { .. } => Ed25519ExternalData::PACKED_PAYLOAD_LEN,
            Self::P256Native { .. } => P256NativeData::PACKED_PAYLOAD_LEN,
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
                permissions,
                data,
                nonce,
            } => {
                let mut payload = Vec::with_capacity(P256WebauthnData::PACKED_PAYLOAD_LEN);
                payload.push(permissions.mask);
                payload.extend_from_slice(&data.compressed_pubkey);
                payload.push(data.rp_id_len);
                payload.extend_from_slice(&data.rp_id);
                payload.extend_from_slice(&data.rp_id_hash);
                payload.extend_from_slice(&data.counter.to_le_bytes());
                payload.extend_from_slice(data.session_key_data.key.as_ref());
                payload.extend_from_slice(&data.session_key_data.expiration.to_le_bytes());
                payload.extend_from_slice(&nonce.to_le_bytes());
                (SignerType::P256Webauthn as u8, payload)
            }
            Self::Secp256k1 {
                permissions,
                data,
                nonce,
            } => {
                let mut payload = Vec::with_capacity(Secp256k1Data::PACKED_PAYLOAD_LEN);
                payload.push(permissions.mask);
                payload.extend_from_slice(&data.uncompressed_pubkey);
                payload.extend_from_slice(&data.eth_address);
                payload.push(data.has_eth_address as u8);
                payload.extend_from_slice(data.session_key_data.key.as_ref());
                payload.extend_from_slice(&data.session_key_data.expiration.to_le_bytes());
                payload.extend_from_slice(&nonce.to_le_bytes());
                (SignerType::Secp256k1 as u8, payload)
            }
            Self::Ed25519External {
                permissions,
                data,
                nonce,
            } => {
                let mut payload = Vec::with_capacity(Ed25519ExternalData::PACKED_PAYLOAD_LEN);
                payload.push(permissions.mask);
                payload.extend_from_slice(&data.external_pubkey);
                payload.extend_from_slice(data.session_key_data.key.as_ref());
                payload.extend_from_slice(&data.session_key_data.expiration.to_le_bytes());
                payload.extend_from_slice(&nonce.to_le_bytes());
                (SignerType::Ed25519External as u8, payload)
            }
            Self::P256Native {
                permissions,
                data,
                nonce,
            } => {
                let mut payload = Vec::with_capacity(P256NativeData::PACKED_PAYLOAD_LEN);
                payload.push(permissions.mask);
                payload.extend_from_slice(&data.compressed_pubkey);
                payload.extend_from_slice(data.session_key_data.key.as_ref());
                payload.extend_from_slice(&data.session_key_data.expiration.to_le_bytes());
                payload.extend_from_slice(&nonce.to_le_bytes());
                (SignerType::P256Native as u8, payload)
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

    /// Parse from packed P256Webauthn payload (155 bytes)
    pub fn from_packed_p256(payload: &[u8]) -> Result<Self> {
        if payload.len() != P256WebauthnData::PACKED_PAYLOAD_LEN {
            return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
        }
        let permissions = Permissions { mask: payload[0] };

        let mut compressed_pubkey = [0u8; 33];
        compressed_pubkey.copy_from_slice(&payload[1..34]);

        let rp_id_len = payload[34];
        if rp_id_len > 32 {
            return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
        }

        let mut rp_id = [0u8; 32];
        rp_id.copy_from_slice(&payload[35..67]);

        let mut rp_id_hash = [0u8; 32];
        rp_id_hash.copy_from_slice(&payload[67..99]);

        let counter = u64::from_le_bytes(
            payload[99..107]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        let session_key = Pubkey::try_from(&payload[107..139])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;

        let session_key_expiration = u64::from_le_bytes(
            payload[139..147]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        let nonce = u64::from_le_bytes(
            payload[147..155]
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
                session_key_data: SessionKeyData {
                    key: session_key,
                    expiration: session_key_expiration,
                },
            },
            nonce,
        })
    }

    /// Parse from packed Secp256k1 payload (134 bytes)
    pub fn from_packed_secp256k1(payload: &[u8]) -> Result<Self> {
        if payload.len() != Secp256k1Data::PACKED_PAYLOAD_LEN {
            return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
        }
        let permissions = Permissions { mask: payload[0] };

        let mut uncompressed_pubkey = [0u8; 64];
        uncompressed_pubkey.copy_from_slice(&payload[1..65]);

        let mut eth_address = [0u8; 20];
        eth_address.copy_from_slice(&payload[65..85]);

        let has_eth_address = payload[85] != 0;

        let session_key = Pubkey::try_from(&payload[86..118])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;

        let session_key_expiration = u64::from_le_bytes(
            payload[118..126]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        let nonce = u64::from_le_bytes(
            payload[126..134]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        Ok(Self::Secp256k1 {
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
            nonce,
        })
    }

    /// Parse from packed P256Native payload (82 bytes)
    pub fn from_packed_p256_native(payload: &[u8]) -> Result<Self> {
        if payload.len() != P256NativeData::PACKED_PAYLOAD_LEN {
            return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
        }
        let permissions = Permissions { mask: payload[0] };

        let mut compressed_pubkey = [0u8; 33];
        compressed_pubkey.copy_from_slice(&payload[1..34]);

        let session_key = Pubkey::try_from(&payload[34..66])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;

        let session_key_expiration = u64::from_le_bytes(
            payload[66..74]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        let nonce = u64::from_le_bytes(
            payload[74..82]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        Ok(Self::P256Native {
            permissions,
            data: P256NativeData {
                compressed_pubkey,
                session_key_data: SessionKeyData {
                    key: session_key,
                    expiration: session_key_expiration,
                },
            },
            nonce,
        })
    }

    /// Parse from packed Ed25519External payload (81 bytes)
    pub fn from_packed_ed25519_external(payload: &[u8]) -> Result<Self> {
        if payload.len() != Ed25519ExternalData::PACKED_PAYLOAD_LEN {
            return Err(error!(crate::errors::SmartAccountError::InvalidPayload));
        }
        let permissions = Permissions { mask: payload[0] };

        let mut external_pubkey = [0u8; 32];
        external_pubkey.copy_from_slice(&payload[1..33]);

        let session_key = Pubkey::try_from(&payload[33..65])
            .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?;

        let session_key_expiration = u64::from_le_bytes(
            payload[65..73]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        let nonce = u64::from_le_bytes(
            payload[73..81]
                .try_into()
                .map_err(|_| error!(crate::errors::SmartAccountError::InvalidPayload))?,
        );

        Ok(Self::Ed25519External {
            permissions,
            data: Ed25519ExternalData {
                external_pubkey,
                session_key_data: SessionKeyData {
                    key: session_key,
                    expiration: session_key_expiration,
                },
            },
            nonce,
        })
    }
}
