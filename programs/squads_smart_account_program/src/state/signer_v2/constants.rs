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
