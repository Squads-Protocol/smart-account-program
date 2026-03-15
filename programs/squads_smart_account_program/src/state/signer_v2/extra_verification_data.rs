use anchor_lang::prelude::*;
use super::precompile::ClientDataJsonReconstructionParams;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum ExtraVerificationData {
    /// P256Webauthn via precompile — 3 bytes reconstruction params.
    /// Signature lives in the precompile instruction at ix[0].
    P256WebauthnPrecompile {
        client_data_params: ClientDataJsonReconstructionParams,
    },

    /// Ed25519External via precompile — no extra data needed.
    /// Signature lives in the precompile instruction at ix[0].
    Ed25519Precompile,

    /// Secp256k1 via precompile — no extra data needed.
    /// Signature lives in the precompile instruction at ix[0].
    Secp256k1Precompile,

    /// P256Native via precompile — no extra data needed (raw message hash, no WebAuthn wrapping).
    /// Signature lives in the precompile instruction at ix[0].
    P256NativePrecompile,

    /// Ed25519External via syscall — 64-byte signature carried inline.
    Ed25519Syscall {
        signature: [u8; 64],
    },

    /// Secp256k1 via syscall — 64-byte signature + 1-byte recovery ID.
    Secp256k1Syscall {
        signature: [u8; 64],
        recovery_id: u8,
    },
}

impl ExtraVerificationData {
    pub fn as_client_data_params(&self) -> Option<&ClientDataJsonReconstructionParams> {
        match self {
            Self::P256WebauthnPrecompile { client_data_params } => Some(client_data_params),
            _ => None,
        }
    }

    pub fn as_ed25519_signature(&self) -> Option<&[u8; 64]> {
        match self {
            Self::Ed25519Syscall { signature } => Some(signature),
            _ => None,
        }
    }

    pub fn as_secp256k1_signature(&self) -> Option<(&[u8; 64], u8)> {
        match self {
            Self::Secp256k1Syscall { signature, recovery_id } => Some((signature, *recovery_id)),
            _ => None,
        }
    }

    pub fn is_precompile(&self) -> bool {
        matches!(
            self,
            Self::P256WebauthnPrecompile { .. }
                | Self::Ed25519Precompile
                | Self::Secp256k1Precompile
                | Self::P256NativePrecompile
        )
    }

    pub fn is_syscall(&self) -> bool {
        matches!(self, Self::Ed25519Syscall { .. } | Self::Secp256k1Syscall { .. })
    }

    /// Deserialize a single ExtraVerificationData from Option<&[u8]>.
    /// Returns None if input is None. Used by async instruction handlers.
    pub fn deserialize_single(bytes: Option<&[u8]>) -> std::result::Result<Option<Self>, ()> {
        match bytes {
            Some(data) => Self::try_from_slice(data).map(Some).map_err(|_| ()),
            None => Ok(None),
        }
    }
}
