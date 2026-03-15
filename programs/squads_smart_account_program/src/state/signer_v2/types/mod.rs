use anchor_lang::prelude::*;

// ============================================================================
// V2 Signer Type
// ============================================================================

/// V2 signer type discriminator (explicit u8 values for stability)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum SignerType {
    Native = 0,
    P256Webauthn = 1,
    Secp256k1 = 2,
    Ed25519External = 3,
    P256Native = 4,
}

mod external_signer_data;
mod session_key;
mod webauthn;
mod secp256k1;
mod ed25519_external;
mod p256_native;
mod signer;
mod signer_packed;
mod signer_raw;

pub use external_signer_data::*;
pub use session_key::*;
pub use webauthn::*;
pub use secp256k1::*;
pub use ed25519_external::*;
pub use p256_native::*;
pub use signer::*;
