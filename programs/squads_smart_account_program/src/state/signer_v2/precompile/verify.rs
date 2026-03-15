use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash;

use crate::errors::SmartAccountError;
use super::{
    reconstruct_client_data_json,
    AuthDataParser,
    parse_precompile_signature,
    get_precompile_num_signatures,
};
use super::ClientDataJsonReconstructionParams;
use super::super::{
    ExtraVerificationData,
    SignerType,
    SmartAccountSigner,
    WEBAUTHN_SIGNATURE_MIN_SIZE,
};

/// Try to verify a single external signer against the precompile instruction at index 0.
///
/// Returns:
/// - `Ok(Some((counter_update, next_nonce)))` — signer found in precompile and verified
/// - `Ok(None)` — signer's pubkey not found in the precompile instruction
/// - `Err(...)` — verification failed (bad message, invalid data, etc.)
///
/// # Precompile Constraint
/// Only one precompile instruction per transaction, always at instruction index 0.
/// That instruction can pack multiple signatures, but only of the same signer type
/// (e.g. multiple secp256r1 signatures, NOT a mix of secp256r1 and ed25519).
///
/// # Arguments
/// * `sysvar` - Instructions sysvar account containing precompile signatures
/// * `signer` - The external signer to verify
/// * `non_hashed_message` - The base message hasher (nonce will be appended)
/// * `extra_verification_data` - Optional extra data for verification, interpreted based on signer type:
///   - P256Webauthn: Parsed as `ClientDataJsonReconstructionParams` (3 bytes)
///   - Secp256k1/Ed25519External: Currently unused, reserved for future use
pub fn try_verify_external_signer(
    sysvar: &AccountInfo,
    signer: &SmartAccountSigner,
    non_hashed_message: &anchor_lang::solana_program::hash::Hasher,
    extra_verification_data: Option<&[u8]>,
) -> Result<Option<(Option<u64>, u64)>> {
    use anchor_lang::solana_program::sysvar::instructions::load_instruction_at_checked;
    use anchor_lang::solana_program::{ed25519_program, secp256k1_program};

    // Get current nonce from signer
    let nonce = signer
        .nonce()
        .ok_or_else(|| error!(SmartAccountError::InvalidSignerType))?;

    // Compute next nonce
    let next_nonce = nonce
        .checked_add(1)
        .ok_or_else(|| error!(SmartAccountError::NonceExhausted))?;

    // Hash message with nonce
    let mut message_with_nonce = non_hashed_message.clone();
    message_with_nonce.hash(&next_nonce.to_le_bytes());
    let expected_message = message_with_nonce.result().to_bytes();

    // Determine which precompile program we're looking for
    let signer_type = signer.signer_type();
    let expected_program_id = match signer_type {
        SignerType::P256Webauthn | SignerType::P256Native => super::SECP256R1_PROGRAM_ID,
        SignerType::Secp256k1 => secp256k1_program::ID,
        SignerType::Ed25519External => ed25519_program::ID,
        SignerType::Native => return Err(SmartAccountError::InvalidSignerType.into()),
    };

    // The single precompile instruction is always at index 0.
    let ix = load_instruction_at_checked(0, sysvar)
        .map_err(|_| SmartAccountError::MissingPrecompileInstruction)?;

    // If the instruction at index 0 isn't our expected precompile, signer is not precompile-verified.
    if ix.program_id != expected_program_id {
        return Ok(None);
    }

    // Stored pubkey for comparison
    let stored_pubkey = signer
        .get_public_key_bytes()
        .ok_or(SmartAccountError::InvalidSignerType)?;

    // Iterate all signatures packed in the single precompile instruction
    // to find the one matching this signer's pubkey.
    let num_sigs = get_precompile_num_signatures(&ix.data, signer_type)?;
    for sig_idx in 0..num_sigs {
        let parsed = parse_precompile_signature(&ix, sysvar, signer_type, sig_idx)?;

        let pubkey_matches = match signer_type {
            SignerType::P256Webauthn | SignerType::P256Native => parsed.public_key.as_slice() == stored_pubkey,
            SignerType::Secp256k1 => {
                // Precompile extracts eth_address (20 bytes). Derive from stored
                // uncompressed pubkey to compare.
                let stored_eth_address = compute_eth_address(stored_pubkey);
                parsed.public_key.as_slice() == &stored_eth_address
            }
            SignerType::Ed25519External => parsed.public_key.as_slice() == stored_pubkey,
            SignerType::Native => false,
        };

        if !pubkey_matches {
            continue;
        }

        let verification = verify_signed_message(&parsed.message, &expected_message, signer, extra_verification_data)?;
        return Ok(Some((verification.new_counter, next_nonce)));
    }

    Ok(None)
}

/// Verify a single external signer against precompile signatures (errors if not found).
///
/// Wrapper around `try_verify_external_signer` for the async path where
/// a missing precompile match is always an error.
pub fn verify_external_signer(
    sysvar: &AccountInfo,
    signer: &SmartAccountSigner,
    non_hashed_message: &anchor_lang::solana_program::hash::Hasher,
    extra_verification_data: Option<&[u8]>,
) -> Result<(Option<u64>, u64)> {
    try_verify_external_signer(sysvar, signer, non_hashed_message, extra_verification_data)?
        .ok_or_else(|| SmartAccountError::MissingPrecompileInstruction.into())
}

/// Batch-verify external signers against the precompile instruction at ix[0].
///
/// Loads the instruction ONCE and verifies all signers by direct index —
/// signature at position `i` in the packed precompile data corresponds to
/// `signers[i]`. No pubkey scanning needed.
///
/// # Arguments
/// * `sysvar` - Instructions sysvar containing the precompile instruction at index 0
/// * `signers` - External signers to verify (must match packed signature order)
/// * `evd` - Typed verification data per signer (1:1 with signers)
/// * `non_hashed_message` - Base message hasher (nonce appended per signer)
///
/// # Returns
/// Vec of `(counter_update, next_nonce)` per signer, in order.
pub fn verify_precompile_signers(
    sysvar: &AccountInfo,
    signers: &[SmartAccountSigner],
    evd: &[ExtraVerificationData],
    non_hashed_message: &anchor_lang::solana_program::hash::Hasher,
) -> Result<Vec<(Option<u64>, u64)>> {
    use anchor_lang::solana_program::sysvar::instructions::load_instruction_at_checked;

    require!(!signers.is_empty(), SmartAccountError::InvalidSignerCount);
    require!(signers.len() == evd.len(), SmartAccountError::InvalidPayload);

    // Validate all signers use the same precompile program.
    // P256Webauthn and P256Native both use SECP256R1, so they can share a batch.
    let first_precompile = precompile_program_for_signer_type(signers[0].signer_type())
        .ok_or(SmartAccountError::InvalidSignerType)?;
    for signer in &signers[1..] {
        let precompile = precompile_program_for_signer_type(signer.signer_type())
            .ok_or(SmartAccountError::InvalidSignerType)?;
        require!(
            precompile == first_precompile,
            SmartAccountError::InvalidSignerType
        );
    }

    // Use the first signer's type for parsing (all share the same precompile format)
    let signer_type = signers[0].signer_type();

    let ix = load_instruction_at_checked(0, sysvar)
        .map_err(|_| SmartAccountError::MissingPrecompileInstruction)?;

    let num_sigs = get_precompile_num_signatures(&ix.data, signer_type)?;
    require!(
        num_sigs == signers.len(),
        SmartAccountError::InvalidSignerCount
    );

    let mut results = Vec::with_capacity(signers.len());

    for (sig_idx, (signer, evd_entry)) in signers.iter().zip(evd.iter()).enumerate() {
        let parsed = parse_precompile_signature(&ix, sysvar, signer_type, sig_idx)?;

        // CRITICAL: Verify the public key in the precompile instruction matches the stored signer.
        // Without this check, an attacker could submit signatures from their own keys.
        let stored_pubkey = signer
            .get_public_key_bytes()
            .ok_or(SmartAccountError::InvalidSignerType)?;
        let per_signer_type = signer.signer_type();
        let pubkey_matches = match per_signer_type {
            SignerType::P256Webauthn | SignerType::P256Native => parsed.public_key.as_slice() == stored_pubkey,
            SignerType::Secp256k1 => {
                let stored_eth = compute_eth_address(stored_pubkey);
                parsed.public_key.as_slice() == &stored_eth
            }
            SignerType::Ed25519External => parsed.public_key.as_slice() == stored_pubkey,
            SignerType::Native => false,
        };
        require!(pubkey_matches, SmartAccountError::PrecompileMessageMismatch);

        // Nonce
        let nonce = signer
            .nonce()
            .ok_or_else(|| error!(SmartAccountError::InvalidSignerType))?;
        let next_nonce = nonce
            .checked_add(1)
            .ok_or_else(|| error!(SmartAccountError::NonceExhausted))?;

        // Hash message with nonce
        let mut message_with_nonce = non_hashed_message.clone();
        message_with_nonce.hash(&next_nonce.to_le_bytes());
        let expected_message = message_with_nonce.result().to_bytes();

        #[cfg(feature = "testing")]
        msg!("precompile_batch: sig_idx={}, nonce={}, next_nonce={}", sig_idx, nonce, next_nonce);

        // Per-signer EVD extraction (P256 needs client_data_params as bytes)
        let serialized;
        let per_signer_bytes = match evd_entry {
            ExtraVerificationData::P256WebauthnPrecompile { client_data_params } => {
                serialized = client_data_params
                    .try_to_vec()
                    .map_err(|_| SmartAccountError::InvalidPayload)?;
                Some(serialized.as_slice())
            }
            _ => None,
        };

        let verification = verify_signed_message(
            &parsed.message,
            &expected_message,
            signer,
            per_signer_bytes,
        )?;

        results.push((verification.new_counter, next_nonce));
    }

    Ok(results)
}

/// Load the precompile instruction at index 0 and return its signature count and signer type.
///
/// Returns `Ok(None)` if the instruction at index 0 is not a recognized precompile program.
/// Used by the sync path to know exactly how many external signers are precompile-verified.
pub fn get_precompile_info(
    sysvar: &AccountInfo,
) -> Result<Option<(usize, SignerType)>> {
    use anchor_lang::solana_program::sysvar::instructions::load_instruction_at_checked;
    use anchor_lang::solana_program::{ed25519_program, secp256k1_program};

    let ix = load_instruction_at_checked(0, sysvar)
        .map_err(|_| SmartAccountError::MissingPrecompileInstruction)?;

    let signer_type = if ix.program_id == super::SECP256R1_PROGRAM_ID {
        SignerType::P256Webauthn
    } else if ix.program_id == secp256k1_program::ID {
        SignerType::Secp256k1
    } else if ix.program_id == ed25519_program::ID {
        SignerType::Ed25519External
    } else {
        return Ok(None);
    };

    let num_sigs = get_precompile_num_signatures(&ix.data, signer_type)?;
    Ok(Some((num_sigs, signer_type)))
}

fn compute_eth_address(pubkey: &[u8]) -> [u8; 20] {
    super::super::secp256k1_syscall::compute_eth_address(pubkey)
}

/// Map a signer type to its precompile program ID.
/// Returns None for Native (not a precompile-verified type).
fn precompile_program_for_signer_type(signer_type: SignerType) -> Option<Pubkey> {
    use anchor_lang::solana_program::{ed25519_program, secp256k1_program};
    match signer_type {
        SignerType::P256Webauthn | SignerType::P256Native => Some(super::SECP256R1_PROGRAM_ID),
        SignerType::Secp256k1 => Some(secp256k1_program::ID),
        SignerType::Ed25519External => Some(ed25519_program::ID),
        SignerType::Native => None,
    }
}

/// Result of verifying a signed message, includes optional counter update for WebAuthn
struct SignedMessageVerification {
    /// New counter value for WebAuthn signers (None for other signer types)
    new_counter: Option<u64>,
}

/// Verify a signed message using the correct flow for each signer type
///
/// For WebAuthn/P256 signers:
/// - The signed message format is: authenticatorData || clientDataHash
/// - We split at (len - 32) to get auth_data and client_data_hash
/// - We reconstruct the expected clientDataJSON and hash it
/// - We compare the hashes to verify the challenge matches
/// - extra_verification_data is parsed as ClientDataJsonReconstructionParams
///
/// For other external signers (Ed25519, Secp256k1):
/// - Direct message comparison
/// - extra_verification_data is currently unused (reserved for future precompiles)
fn verify_signed_message(
    signed: &[u8],
    expected_challenge: &[u8],
    signer: &SmartAccountSigner,
    extra_verification_data: Option<&[u8]>,
) -> Result<SignedMessageVerification> {
    match signer {
        SmartAccountSigner::P256Webauthn { data, .. } => {
            // Minimum size: authenticatorData (37 min) + clientDataHash (32) = 69 bytes
            require!(signed.len() >= WEBAUTHN_SIGNATURE_MIN_SIZE, SmartAccountError::InvalidPrecompileData);

            // Split the message: authenticatorData || clientDataHash
            // clientDataHash is always the last 32 bytes
            let (auth_data, client_data_hash) = signed.split_at(signed.len() - 32);

            // Parse authenticator data using AuthDataParser
            let auth_parser = AuthDataParser::new(auth_data)?;

            // Validate rpIdHash
            require!(
                auth_parser.rp_id_hash() == data.rp_id_hash.as_slice(),
                SmartAccountError::WebauthnRpIdMismatch
            );

            // Check user presence flag
            require!(
                auth_parser.is_user_present(),
                SmartAccountError::WebauthnUserNotPresent
            );

            // Parse and validate counter (replay protection)
            // WebAuthn counters are monotonically increasing - each signature MUST have
            // a counter strictly greater than the previously stored counter.
            // Exception: When both are 0, this is the first use and we accept it.
            let sign_counter = auth_parser.get_counter() as u64;
            if data.counter > 0 || sign_counter > 0 {
                require!(
                    sign_counter > data.counter,
                    SmartAccountError::WebauthnCounterNotIncremented
                );
            }

            // Parse extra_verification_data as ClientDataJsonReconstructionParams for WebAuthn
            let extra_data = extra_verification_data
                .ok_or(SmartAccountError::MissingClientDataParams)?;
            let params = ClientDataJsonReconstructionParams::try_from_slice(extra_data)
                .map_err(|_| SmartAccountError::MissingClientDataParams)?;
            require!(
                params.is_valid_type(),
                SmartAccountError::InvalidPayload
            );

            let rp_id = data.get_rp_id();
            let reconstructed_client_data = reconstruct_client_data_json(&params, rp_id, expected_challenge)?;

            // Hash the reconstructed clientDataJSON
            let reconstructed_hash = hash(&reconstructed_client_data);

            // Compare hashes
            require!(
                client_data_hash == reconstructed_hash.to_bytes().as_slice(),
                SmartAccountError::PrecompileMessageMismatch
            );

            // Persist the counter for replay protection.
            // If both counters are 0 (first use), persist as 1 to distinguish
            // "used once with counter 0" from "never used", preventing the
            // first-use authentication from being accepted again.
            let persisted_counter = if sign_counter == 0 && data.counter == 0 {
                1
            } else {
                sign_counter
            };
            Ok(SignedMessageVerification { new_counter: Some(persisted_counter) })
        }
        _ => {
            // For non-WebAuthn signers, direct comparison
            // extra_verification_data is ignored (reserved for future precompiles/curves)
            require!(
                signed == expected_challenge,
                SmartAccountError::PrecompileMessageMismatch
            );
            Ok(SignedMessageVerification { new_counter: None })
        }
    }
}

/// Update the WebAuthn counter for a signer after successful verification
///
/// This should be called after signature verification to persist the new counter.
/// The caller must have mutable access to the signer's data.
pub fn update_webauthn_counter(
    signer: &mut SmartAccountSigner,
    new_counter: u64,
) -> Result<()> {
    match signer {
        SmartAccountSigner::P256Webauthn { data, .. } => {
            data.counter = new_counter;
            Ok(())
        }
        _ => Err(SmartAccountError::InvalidSignerType.into()),
    }
}

/// Helper: if external signatures are used, instructions sysvar must be the first remaining account
pub fn split_instructions_sysvar<'a, 'info>(
    remaining_accounts: &'a [AccountInfo<'info>],
) -> (Option<&'a AccountInfo<'info>>, &'a [AccountInfo<'info>]) {
    match remaining_accounts.first() {
        Some(acc) if acc.key == &anchor_lang::solana_program::sysvar::instructions::ID => {
            (Some(acc), &remaining_accounts[1..])
        }
        _ => (None, remaining_accounts),
    }
}
