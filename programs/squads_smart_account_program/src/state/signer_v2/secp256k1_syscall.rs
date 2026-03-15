/// Secp256k1 signature verification using Solana secp256k1_recover syscall.
///
/// Recovers the public key from a secp256k1 signature and verifies it matches
/// the expected Ethereum address. Uses keccak256 for both message hashing and
/// eth_address derivation.
///
/// Cost: ~25,000 CU per verification (secp256k1_recover syscall + 2 keccak hashes).

use anchor_lang::solana_program::keccak::hash as keccak256;

#[derive(Debug)]
pub enum Secp256k1SyscallError {
    InvalidArgument,
    InvalidSignature,
    RecoveryFailed,
    AddressMismatch,
}

/// Compute Ethereum address from an uncompressed secp256k1 public key.
///
/// eth_address = keccak256(pubkey)[12..32]
///
/// This is used by both the precompile path (to compare against parsed data)
/// and the syscall path (to compare against recovered key).
pub fn compute_eth_address(pubkey: &[u8]) -> [u8; 20] {
    let hash = keccak256(pubkey);
    let mut eth_address = [0u8; 20];
    eth_address.copy_from_slice(&hash.0[12..32]);
    eth_address
}

/// Verify a secp256k1 signature using the Solana secp256k1_recover syscall.
///
/// # Arguments
/// * `eth_address` - Expected 20-byte Ethereum address
/// * `signature` - 64-byte secp256k1 signature (r || s)
/// * `recovery_id` - Recovery ID (0-3)
/// * `message` - Message that was signed (will be keccak256-hashed before recovery)
pub fn secp256k1_syscall_verify(
    eth_address: &[u8; 20],
    signature: &[u8],
    recovery_id: u8,
    message: &[u8],
) -> Result<(), Secp256k1SyscallError> {
    use anchor_lang::solana_program::secp256k1_recover::secp256k1_recover;

    if signature.len() != 64 {
        return Err(Secp256k1SyscallError::InvalidArgument);
    }
    if recovery_id >= 4 {
        return Err(Secp256k1SyscallError::InvalidArgument);
    }

    // Hash the message with keccak256 (Ethereum signing convention)
    let message_hash = keccak256(message);

    // Recover public key via secp256k1_recover syscall
    let recovered_pubkey = secp256k1_recover(
        &message_hash.0,
        recovery_id,
        signature,
    )
    .map_err(|_| Secp256k1SyscallError::RecoveryFailed)?;

    // Derive eth_address from recovered pubkey
    let recovered_eth_address = compute_eth_address(&recovered_pubkey.0);

    if recovered_eth_address == *eth_address {
        Ok(())
    } else {
        #[cfg(feature = "testing")]
        {
            anchor_lang::prelude::msg!("k1_syscall: eth_address mismatch");
            anchor_lang::prelude::msg!("expected: {:?}", &eth_address[..8]);
            anchor_lang::prelude::msg!("recovered: {:?}", &recovered_eth_address[..8]);
        }
        Err(Secp256k1SyscallError::AddressMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_eth_address_known_vector() {
        // All-zero pubkey → known keccak256 hash
        let pubkey = [0u8; 64];
        let eth_address = compute_eth_address(&pubkey);
        // keccak256 of 64 zero bytes, take last 20
        let hash = keccak256(&pubkey);
        assert_eq!(&eth_address, &hash.0[12..32]);
        assert_eq!(eth_address.len(), 20);
    }

    #[test]
    fn test_compute_eth_address_deterministic() {
        let pubkey = [0xAB; 64];
        let addr1 = compute_eth_address(&pubkey);
        let addr2 = compute_eth_address(&pubkey);
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_compute_eth_address_different_keys() {
        let addr1 = compute_eth_address(&[0xAA; 64]);
        let addr2 = compute_eth_address(&[0xBB; 64]);
        assert_ne!(addr1, addr2);
    }

    #[test]
    fn test_secp256k1_invalid_signature_length() {
        let eth_address = [0u8; 20];
        let short_sig = [0u8; 63];
        let result = secp256k1_syscall_verify(&eth_address, &short_sig, 0, b"test");
        assert!(matches!(result, Err(Secp256k1SyscallError::InvalidArgument)));
    }

    #[test]
    fn test_secp256k1_invalid_recovery_id() {
        let eth_address = [0u8; 20];
        let sig = [0u8; 64];
        let result = secp256k1_syscall_verify(&eth_address, &sig, 4, b"test");
        assert!(matches!(result, Err(Secp256k1SyscallError::InvalidArgument)));

        let result = secp256k1_syscall_verify(&eth_address, &sig, 255, b"test");
        assert!(matches!(result, Err(Secp256k1SyscallError::InvalidArgument)));
    }

    #[test]
    fn test_secp256k1_recovery_failure_invalid_sig() {
        let eth_address = [0u8; 20];
        let sig = [0u8; 64]; // all-zero signature is invalid
        let result = secp256k1_syscall_verify(&eth_address, &sig, 0, b"test");
        // Should fail at recovery (invalid signature)
        assert!(result.is_err());
    }
}
