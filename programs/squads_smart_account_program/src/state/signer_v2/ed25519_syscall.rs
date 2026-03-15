/// Ed25519 signature verification using Solana curve25519 syscalls.
///
/// Vendored from brine-ed25519 (https://github.com/zfedoran/brine-ed25519)
/// because brine-ed25519 depends on solana-curve25519 ^2.0.13 which pins
/// solana-program =2.0.13 (incompatible with our 1.17.4), and uses
/// curve25519-dalek ^4.1.3 (we have v3.2.1).
///
/// Uses raw sol_curve_group_op / sol_curve_validate_point syscalls from
/// solana_program::syscalls and curve25519-dalek v3 for scalar operations.
///
/// Cost: ~40,000-50,000 CU per verification (3 syscall point operations + SHA-512).

use sha2::{Digest, Sha512};
use curve25519_dalek::scalar::Scalar;

const ED25519_SIG_LEN: usize = 64;
const ED25519_PUBKEY_LEN: usize = 32;

/// Compressed Edwards base point (generator G).
const G: [u8; 32] = [
    88, 102, 102, 102, 102, 102, 102, 102, 102, 102, 102, 102, 102, 102, 102, 102,
    102, 102, 102, 102, 102, 102, 102, 102, 102, 102, 102, 102, 102, 102, 102, 102,
];

/// Curve ID for Edwards curve in Solana syscalls.
const CURVE25519_EDWARDS: u64 = 0;

/// Group operation: multiply (scalar * point).
const MUL: u64 = 2;

/// Group operation: subtract (point - point).
const SUB: u64 = 1;

/// 32-byte compressed Edwards point wrapper.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct PodEdwardsPoint([u8; 32]);

/// 32-byte scalar wrapper.
#[repr(transparent)]
struct PodScalar([u8; 32]);

#[derive(Debug)]
pub enum Ed25519SyscallError {
    InvalidArgument,
    InvalidPublicKey,
    InvalidSignature,
}

/// Verify an Ed25519 signature using Solana curve25519 syscalls.
///
/// # Arguments
/// * `pubkey` - 32-byte Ed25519 public key
/// * `sig` - 64-byte Ed25519 signature
/// * `message` - arbitrary-length message
#[allow(non_snake_case)]
pub fn ed25519_syscall_verify(pubkey: &[u8], sig: &[u8], message: &[u8]) -> Result<(), Ed25519SyscallError> {
    if pubkey.len() != ED25519_PUBKEY_LEN {
        return Err(Ed25519SyscallError::InvalidArgument);
    }
    if sig.len() != ED25519_SIG_LEN {
        return Err(Ed25519SyscallError::InvalidArgument);
    }

    let pubkey_point = PodEdwardsPoint(
        pubkey[..ED25519_PUBKEY_LEN]
            .try_into()
            .map_err(|_| Ed25519SyscallError::InvalidArgument)?,
    );

    // Split signature: lower 32 bytes = R, upper 32 bytes = s
    let mut sig_lower = [0u8; 32];
    let mut sig_upper = [0u8; 32];
    sig_lower.copy_from_slice(&sig[..32]);
    sig_upper.copy_from_slice(&sig[32..]);

    let sig_R = PodEdwardsPoint(sig_lower);
    // curve25519-dalek v3: from_canonical_bytes returns Option<Scalar>
    let sig_s = match Scalar::from_canonical_bytes(sig_upper) {
        Some(s) => s,
        None => {
            #[cfg(feature = "testing")]
            anchor_lang::prelude::msg!("ed25519: from_canonical_bytes failed for s");
            return Err(Ed25519SyscallError::InvalidSignature);
        }
    };

    // Reject small-order points (cofactor attack prevention)
    if is_small_order(&sig_R) {
        #[cfg(feature = "testing")]
        anchor_lang::prelude::msg!("ed25519: sig_R is small order");
        return Err(Ed25519SyscallError::InvalidPublicKey);
    }
    if is_small_order(&pubkey_point) {
        #[cfg(feature = "testing")]
        anchor_lang::prelude::msg!("ed25519: pubkey is small order");
        return Err(Ed25519SyscallError::InvalidPublicKey);
    }

    // Validate points are on the curve
    if !validate_edwards(&pubkey_point) {
        #[cfg(feature = "testing")]
        anchor_lang::prelude::msg!("ed25519: pubkey not on curve");
        return Err(Ed25519SyscallError::InvalidPublicKey);
    }
    if !validate_edwards(&sig_R) {
        #[cfg(feature = "testing")]
        anchor_lang::prelude::msg!("ed25519: sig_R not on curve");
        return Err(Ed25519SyscallError::InvalidPublicKey);
    }

    // Hash(R || pubkey || message) → scalar k
    let mut h = Sha512::new();
    h.update(sig_R.0);
    h.update(pubkey);
    h.update(message);
    let f = h.finalize();

    // curve25519-dalek v3: from_bytes_mod_order_wide takes &[u8; 64]
    let hash_bytes: [u8; 64] = f
        .as_slice()
        .try_into()
        .map_err(|_| Ed25519SyscallError::InvalidArgument)?;
    let k = Scalar::from_bytes_mod_order_wide(&hash_bytes);

    let a = PodScalar(k.to_bytes());
    let b = PodScalar(sig_s.to_bytes());
    let B = PodEdwardsPoint(G);

    // R = sB - kA
    let sB = multiply_edwards(&b, &B)
        .ok_or(Ed25519SyscallError::InvalidSignature)?;
    let kA = multiply_edwards(&a, &pubkey_point)
        .ok_or(Ed25519SyscallError::InvalidSignature)?;
    let R = subtract_edwards(&sB, &kA)
        .ok_or(Ed25519SyscallError::InvalidSignature)?;

    if sig_R.0 == R.0 {
        Ok(())
    } else {
        #[cfg(feature = "testing")]
        {
            anchor_lang::prelude::msg!("ed25519_syscall: sig_R != computed R");
            anchor_lang::prelude::msg!("sig_R: {:?}", &sig_R.0[..16]);
            anchor_lang::prelude::msg!("comp_R: {:?}", &R.0[..16]);
            anchor_lang::prelude::msg!("msg_len: {}", message.len());
        }
        Err(Ed25519SyscallError::InvalidSignature)
    }
}

fn is_small_order(point: &PodEdwardsPoint) -> bool {
    let scalar_8 = {
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&8u64.to_le_bytes());
        PodScalar(bytes)
    };
    if let Some(result) = multiply_edwards(&scalar_8, point) {
        // Identity point in compressed Edwards form
        let mut identity = [0u8; 32];
        identity[0] = 1;
        result.0 == identity
    } else {
        // Conservative: if multiply fails, treat as small-order (reject)
        true
    }
}

// Syscall wrappers — these call sol_curve_group_op / sol_curve_validate_point
// directly from solana_program::syscalls, avoiding the solana-curve25519 crate.

#[cfg(target_os = "solana")]
fn validate_edwards(point: &PodEdwardsPoint) -> bool {
    let mut validate_result = 0u8;
    let result = unsafe {
        solana_program::syscalls::sol_curve_validate_point(
            CURVE25519_EDWARDS,
            &point.0 as *const u8,
            &mut validate_result,
        )
    };
    result == 0
}

#[cfg(target_os = "solana")]
fn multiply_edwards(scalar: &PodScalar, point: &PodEdwardsPoint) -> Option<PodEdwardsPoint> {
    let mut result_point = PodEdwardsPoint([0u8; 32]);
    let result = unsafe {
        solana_program::syscalls::sol_curve_group_op(
            CURVE25519_EDWARDS,
            MUL,
            &scalar.0 as *const u8,
            &point.0 as *const u8,
            &mut result_point.0 as *mut u8,
        )
    };
    if result == 0 { Some(result_point) } else { None }
}

#[cfg(target_os = "solana")]
fn subtract_edwards(left: &PodEdwardsPoint, right: &PodEdwardsPoint) -> Option<PodEdwardsPoint> {
    let mut result_point = PodEdwardsPoint([0u8; 32]);
    let result = unsafe {
        solana_program::syscalls::sol_curve_group_op(
            CURVE25519_EDWARDS,
            SUB,
            &left.0 as *const u8,
            &right.0 as *const u8,
            &mut result_point.0 as *mut u8,
        )
    };
    if result == 0 { Some(result_point) } else { None }
}

// Non-Solana target: use curve25519-dalek for tests
#[cfg(not(target_os = "solana"))]
fn validate_edwards(point: &PodEdwardsPoint) -> bool {
    use curve25519_dalek::edwards::CompressedEdwardsY;
    CompressedEdwardsY::from_slice(&point.0).decompress().is_some()
}

#[cfg(not(target_os = "solana"))]
fn multiply_edwards(scalar: &PodScalar, point: &PodEdwardsPoint) -> Option<PodEdwardsPoint> {
    use curve25519_dalek::edwards::CompressedEdwardsY;
    let scalar = Scalar::from_canonical_bytes(scalar.0)?;
    let point = CompressedEdwardsY::from_slice(&point.0).decompress()?;
    let result = scalar * point;
    Some(PodEdwardsPoint(result.compress().to_bytes()))
}

#[cfg(not(target_os = "solana"))]
fn subtract_edwards(left: &PodEdwardsPoint, right: &PodEdwardsPoint) -> Option<PodEdwardsPoint> {
    use curve25519_dalek::edwards::CompressedEdwardsY;
    let left = CompressedEdwardsY::from_slice(&left.0).decompress()?;
    let right = CompressedEdwardsY::from_slice(&right.0).decompress()?;
    let result = left - right;
    Some(PodEdwardsPoint(result.compress().to_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_world() {
        let pubkey: [u8; 32] = [
            73, 73, 170, 112, 75, 235, 154, 81, 203, 8, 44, 245, 233, 18, 204, 136,
            162, 9, 233, 49, 154, 201, 171, 175, 47, 6, 223, 101, 105, 80, 95, 166,
        ];
        let sig: [u8; 64] = [
            164, 121, 89, 242, 88, 29, 80, 177, 104, 20, 102, 176, 48, 133, 68, 8,
            105, 33, 58, 86, 28, 108, 198, 140, 160, 219, 62, 184, 154, 181, 140, 33,
            35, 102, 183, 203, 111, 33, 55, 170, 180, 138, 92, 196, 185, 201, 122, 167,
            15, 112, 9, 228, 226, 112, 111, 10, 142, 73, 85, 43, 81, 152, 204, 13,
        ];
        assert!(ed25519_syscall_verify(&pubkey, &sig, b"hello world").is_ok());
        assert!(ed25519_syscall_verify(&pubkey, &sig, b"wrong message").is_err());
    }

    #[test]
    fn test_rfc8032_vector_1() {
        let pubkey: [u8; 32] = [
            0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
            0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
        ];
        let sig: [u8; 64] = [
            0xe5, 0x56, 0x43, 0x00, 0xc3, 0x60, 0xac, 0x72, 0x90, 0x86, 0xe2, 0xcc, 0x80, 0x6e, 0x82, 0x8a,
            0x84, 0x87, 0x7f, 0x1e, 0xb8, 0xe5, 0xd9, 0x74, 0xd8, 0x73, 0xe0, 0x65, 0x22, 0x49, 0x01, 0x55,
            0x5f, 0xb8, 0x82, 0x15, 0x90, 0xa3, 0x3b, 0xac, 0xc6, 0x1e, 0x39, 0x70, 0x1c, 0xf9, 0xb4, 0x6b,
            0xd2, 0x5b, 0xf5, 0xf0, 0x59, 0x5b, 0xbe, 0x24, 0x65, 0x51, 0x41, 0x43, 0x8e, 0x7a, 0x10, 0x0b,
        ];
        assert!(ed25519_syscall_verify(&pubkey, &sig, b"").is_ok());
    }
}
