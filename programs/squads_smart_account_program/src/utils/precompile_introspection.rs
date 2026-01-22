use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    ed25519_program,
    instruction::Instruction,
    pubkey,
    secp256k1_program,
    sysvar::instructions::{load_current_index_checked, load_instruction_at_checked},
};
use std::collections::HashSet;

use crate::errors::SmartAccountError;
use crate::state::{SignerTypeV2, SmartAccountSignerV2};

pub const SECP256R1_PROGRAM_ID: Pubkey = pubkey!("Secp256r1SigVerify1111111111111111111111111");

#[derive(Debug)]
pub struct ParsedPrecompileSignature {
    pub signer_type: SignerTypeV2,
    pub public_key: Vec<u8>,
    pub message: Vec<u8>,
    pub signature: Vec<u8>,
}

pub struct ExternalSignatureVerification {
    pub verified_key_ids: Vec<Pubkey>,
    pub verified_count: usize,
}

#[derive(Clone, Copy, Debug)]
struct SignatureOffsets {
    signature_offset: u16,
    signature_instruction_index: u16,
    public_key_offset: u16,
    public_key_instruction_index: u16,
    message_data_offset: u16,
    message_data_size: u16,
    message_instruction_index: u16,
}

#[derive(Clone, Copy, Debug)]
struct LegacySecp256k1SignatureOffsets {
    signature_offset: u16,
    signature_instruction_index: u8,
    public_key_offset: u16,
    public_key_instruction_index: u8,
    message_data_offset: u16,
    message_data_size: u16,
    message_instruction_index: u8,
}

impl From<LegacySecp256k1SignatureOffsets> for SignatureOffsets {
    fn from(v: LegacySecp256k1SignatureOffsets) -> Self {
        Self {
            signature_offset: v.signature_offset,
            signature_instruction_index: v.signature_instruction_index as u16,
            public_key_offset: v.public_key_offset,
            public_key_instruction_index: v.public_key_instruction_index as u16,
            message_data_offset: v.message_data_offset,
            message_data_size: v.message_data_size,
            message_instruction_index: v.message_instruction_index as u16,
        }
    }
}

trait PrecompileInfo {
    fn program_id() -> Pubkey;
    fn signature_size() -> usize;
    fn public_key_size() -> usize;
    fn offsets_size() -> usize;
    fn num_signatures_size() -> usize;
    fn data_start_offset() -> usize;
    fn parse_offsets(data: &[u8]) -> Result<SignatureOffsets>;
}

struct Secp256r1;
struct LegacySecp256k1;
struct Ed25519;

impl PrecompileInfo for Secp256r1 {
    fn program_id() -> Pubkey {
        SECP256R1_PROGRAM_ID
    }
    fn signature_size() -> usize {
        64
    }
    fn public_key_size() -> usize {
        33
    }
    fn offsets_size() -> usize {
        14
    }
    fn num_signatures_size() -> usize {
        2
    }
    fn data_start_offset() -> usize {
        Self::num_signatures_size()
    }
    fn parse_offsets(data: &[u8]) -> Result<SignatureOffsets> {
        ensure_len(data, 14)?;
        Ok(SignatureOffsets {
            signature_offset: u16::from_le_bytes([data[0], data[1]]),
            signature_instruction_index: u16::from_le_bytes([data[2], data[3]]),
            public_key_offset: u16::from_le_bytes([data[4], data[5]]),
            public_key_instruction_index: u16::from_le_bytes([data[6], data[7]]),
            message_data_offset: u16::from_le_bytes([data[8], data[9]]),
            message_data_size: u16::from_le_bytes([data[10], data[11]]),
            message_instruction_index: u16::from_le_bytes([data[12], data[13]]),
        })
    }
}

impl PrecompileInfo for LegacySecp256k1 {
    fn program_id() -> Pubkey {
        secp256k1_program::ID
    }
    fn signature_size() -> usize {
        65
    }
    fn public_key_size() -> usize {
        20
    }
    fn offsets_size() -> usize {
        11
    }
    fn num_signatures_size() -> usize {
        1
    }
    fn data_start_offset() -> usize {
        Self::num_signatures_size()
    }
    fn parse_offsets(data: &[u8]) -> Result<SignatureOffsets> {
        ensure_len(data, 11)?;
        let legacy = LegacySecp256k1SignatureOffsets {
            signature_offset: u16::from_le_bytes([data[0], data[1]]),
            signature_instruction_index: data[2],
            public_key_offset: u16::from_le_bytes([data[3], data[4]]),
            public_key_instruction_index: data[5],
            message_data_offset: u16::from_le_bytes([data[6], data[7]]),
            message_data_size: u16::from_le_bytes([data[8], data[9]]),
            message_instruction_index: data[10],
        };
        Ok(legacy.into())
    }
}

impl PrecompileInfo for Ed25519 {
    fn program_id() -> Pubkey {
        ed25519_program::ID
    }
    fn signature_size() -> usize {
        64
    }
    fn public_key_size() -> usize {
        32
    }
    fn offsets_size() -> usize {
        14
    }
    fn num_signatures_size() -> usize {
        2
    }
    fn data_start_offset() -> usize {
        Self::num_signatures_size()
    }
    fn parse_offsets(data: &[u8]) -> Result<SignatureOffsets> {
        ensure_len(data, 14)?;
        Ok(SignatureOffsets {
            signature_offset: u16::from_le_bytes([data[0], data[1]]),
            signature_instruction_index: u16::from_le_bytes([data[2], data[3]]),
            public_key_offset: u16::from_le_bytes([data[4], data[5]]),
            public_key_instruction_index: u16::from_le_bytes([data[6], data[7]]),
            message_data_offset: u16::from_le_bytes([data[8], data[9]]),
            message_data_size: u16::from_le_bytes([data[10], data[11]]),
            message_instruction_index: u16::from_le_bytes([data[12], data[13]]),
        })
    }
}

struct SignaturePayload {
    signature: Vec<u8>,
    public_key: Vec<u8>,
    message: Vec<u8>,
}

fn ensure_len(data: &[u8], len: usize) -> Result<()> {
    require!(data.len() >= len, SmartAccountError::InvalidPrecompileData);
    Ok(())
}

fn ensure_range(data: &[u8], offset: usize, size: usize) -> Result<()> {
    require!(
        offset.checked_add(size).map_or(false, |end| end <= data.len()),
        SmartAccountError::InvalidPrecompileData
    );
    Ok(())
}

/// Get number of signatures from precompile instruction data
fn get_num_signatures<T: PrecompileInfo>(data: &[u8]) -> Result<usize> {
    ensure_len(data, T::num_signatures_size())?;
    let num = match T::num_signatures_size() {
        1 => data[0] as usize,
        2 => u16::from_le_bytes([data[0], data[1]]) as usize,
        _ => return Err(SmartAccountError::InvalidPrecompileData.into()),
    };
    Ok(num)
}

/// Extract signature payload from precompile instruction data
/// 
/// The `instructions_sysvar` is used to load cross-referenced instruction data
/// when the offsets point to a different instruction in the transaction.
fn extract_signature_payload<T: PrecompileInfo>(
    precompile_data: &[u8],
    instructions_sysvar: &AccountInfo,
    index: usize,
) -> Result<SignaturePayload> {
    let num = get_num_signatures::<T>(precompile_data)?;
    require!(index < num, SmartAccountError::InvalidPrecompileData);

    let offsets_start = T::data_start_offset() + T::offsets_size() * index;
    let offsets_end = offsets_start + T::offsets_size();
    ensure_len(precompile_data, offsets_end)?;

    let offsets = T::parse_offsets(&precompile_data[offsets_start..offsets_end])?;

    // Helper to read a slice, potentially from another instruction
    let read_slice = |ix_idx: u16, offset: u16, size: usize| -> Result<Vec<u8>> {
        if ix_idx == u16::MAX {
            // Data is in the current instruction
            ensure_range(precompile_data, offset as usize, size)?;
            Ok(precompile_data[offset as usize..offset as usize + size].to_vec())
        } else {
            // Data is in another instruction - load it
            let ix = load_instruction_at_checked(ix_idx as usize, instructions_sysvar)
                .map_err(|_| SmartAccountError::MissingPrecompileInstruction)?;
            ensure_range(&ix.data, offset as usize, size)?;
            Ok(ix.data[offset as usize..offset as usize + size].to_vec())
        }
    };

    let signature = read_slice(
        offsets.signature_instruction_index,
        offsets.signature_offset,
        T::signature_size(),
    )?;
    let public_key = read_slice(
        offsets.public_key_instruction_index,
        offsets.public_key_offset,
        T::public_key_size(),
    )?;
    let message = read_slice(
        offsets.message_instruction_index,
        offsets.message_data_offset,
        offsets.message_data_size as usize,
    )?;

    Ok(SignaturePayload {
        signature,
        public_key,
        message,
    })
}

/// Parse a precompile signature from instruction data
pub fn parse_precompile_signature(
    precompile_ix: &Instruction,
    instructions_sysvar: &AccountInfo,
    signer_type: SignerTypeV2,
    index: usize,
) -> Result<ParsedPrecompileSignature> {
    // Verify program ID matches expected precompile
    let expected_program_id = match signer_type {
        SignerTypeV2::P256Webauthn => SECP256R1_PROGRAM_ID,
        SignerTypeV2::Secp256k1 => secp256k1_program::ID,
        SignerTypeV2::Ed25519External => ed25519_program::ID,
        SignerTypeV2::Native => return Err(SmartAccountError::InvalidSignerType.into()),
    };
    require_keys_eq!(
        precompile_ix.program_id,
        expected_program_id,
        SmartAccountError::InvalidPrecompileProgram
    );

    let payload = match signer_type {
        SignerTypeV2::P256Webauthn => {
            extract_signature_payload::<Secp256r1>(&precompile_ix.data, instructions_sysvar, index)?
        }
        SignerTypeV2::Secp256k1 => {
            extract_signature_payload::<LegacySecp256k1>(&precompile_ix.data, instructions_sysvar, index)?
        }
        SignerTypeV2::Ed25519External => {
            extract_signature_payload::<Ed25519>(&precompile_ix.data, instructions_sysvar, index)?
        }
        SignerTypeV2::Native => return Err(SmartAccountError::InvalidSignerType.into()),
    };

    Ok(ParsedPrecompileSignature {
        signer_type,
        public_key: payload.public_key,
        message: payload.message,
        signature: payload.signature,
    })
}

/// Verify external signatures from precompile instructions in the transaction
pub fn verify_external_signatures(
    instructions_sysvar: &AccountInfo,
    external_signers: &[SmartAccountSignerV2],
    expected_message: &[u8],
    required_key_ids: Option<&[Pubkey]>,
) -> Result<ExternalSignatureVerification> {
    let current_idx = load_current_index_checked(instructions_sysvar)
        .map_err(|_| SmartAccountError::MissingPrecompileInstruction)? as usize;

    let mut verified_key_ids = Vec::new();
    let mut used_signatures: HashSet<Vec<u8>> = HashSet::new();

    for ix_idx in 0..current_idx {
        let ix = load_instruction_at_checked(ix_idx, instructions_sysvar)
            .map_err(|_| SmartAccountError::MissingPrecompileInstruction)?;

        let signer_type = match &ix.program_id {
            id if id == &SECP256R1_PROGRAM_ID => SignerTypeV2::P256Webauthn,
            id if id == &secp256k1_program::ID => SignerTypeV2::Secp256k1,
            id if id == &ed25519_program::ID => SignerTypeV2::Ed25519External,
            _ => continue,
        };

        // Get number of signatures in this precompile instruction
        let num = match signer_type {
            SignerTypeV2::P256Webauthn => get_num_signatures::<Secp256r1>(&ix.data)?,
            SignerTypeV2::Secp256k1 => get_num_signatures::<LegacySecp256k1>(&ix.data)?,
            SignerTypeV2::Ed25519External => get_num_signatures::<Ed25519>(&ix.data)?,
            SignerTypeV2::Native => 0,
        };

        for sig_idx in 0..num {
            let parsed = parse_precompile_signature(&ix, instructions_sysvar, signer_type, sig_idx)?;

            // Check for duplicate signatures (replay protection)
            if used_signatures.contains(&parsed.signature) {
                return Err(SmartAccountError::DuplicateExternalSignature.into());
            }
            used_signatures.insert(parsed.signature.clone());

            // Match against external signers
            for signer in external_signers.iter() {
                if signer.signer_type() != signer_type {
                    continue;
                }

                if let Some(stored_pubkey) = signer.get_public_key_bytes() {
                    if pubkeys_match(stored_pubkey, &parsed.public_key, signer_type) {
                        verify_signed_message(&parsed.message, expected_message, signer)?;
                        if !verified_key_ids.contains(&signer.key()) {
                            verified_key_ids.push(signer.key());
                        }
                        break;
                    }
                }
            }
        }
    }

    // Verify all required signers are present
    if let Some(required) = required_key_ids {
        for required_key in required {
            require!(
                verified_key_ids.contains(required_key),
                SmartAccountError::MissingPrecompileInstruction
            );
        }
    }

    Ok(ExternalSignatureVerification {
        verified_count: verified_key_ids.len(),
        verified_key_ids,
    })
}

fn pubkeys_match(stored: &[u8], parsed: &[u8], signer_type: SignerTypeV2) -> bool {
    match signer_type {
        SignerTypeV2::P256Webauthn => stored == parsed,
        SignerTypeV2::Secp256k1 => {
            // For secp256k1, stored is uncompressed pubkey (64 bytes), parsed is eth_address (20 bytes)
            if stored.len() != 64 || parsed.len() != 20 {
                return false;
            }
            compute_eth_address(stored).as_slice() == parsed
        }
        SignerTypeV2::Ed25519External => stored == parsed,
        SignerTypeV2::Native => false,
    }
}

fn compute_eth_address(pubkey: &[u8]) -> [u8; 20] {
    use anchor_lang::solana_program::keccak::hash as keccak256;

    let hash = keccak256(pubkey);
    let mut eth_address = [0u8; 20];
    eth_address.copy_from_slice(&hash.0[12..32]);
    eth_address
}

fn verify_signed_message(
    signed: &[u8],
    expected: &[u8],
    signer: &SmartAccountSignerV2,
) -> Result<()> {
    match signer {
        SmartAccountSignerV2::P256Webauthn { data, .. } => {
            // WebAuthn authenticator data format:
            // - bytes 0-31: rpIdHash (32 bytes)
            // - byte 32: flags
            // - bytes 33-36: signCount (4 bytes, big-endian)
            // - remaining: optional extensions and attested credential data
            require!(signed.len() >= 37, SmartAccountError::InvalidPrecompileData);

            let rp_id_hash = &signed[..32];
            require!(
                rp_id_hash == data.rp_id_hash,
                SmartAccountError::WebauthnRpIdMismatch
            );

            let flags = signed[32];
            // Check UP (User Present) flag (bit 0)
            require!(
                (flags & 0x01) != 0,
                SmartAccountError::WebauthnUserNotPresent
            );

            // Note: Full WebAuthn validation should also:
            // - Parse clientDataJSON and verify challenge == base64url(expected)
            // - Verify type == "webauthn.get"
            // - Validate origin if enforced
            // - Enforce counter monotonicity and persist updated counter

            Ok(())
        }
        _ => {
            require!(
                signed == expected,
                SmartAccountError::PrecompileMessageMismatch
            );
            Ok(())
        }
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

/// Create the expected message to sign for a given operation
///
/// Format: hash("squads-v2" || smart_account_key || operation_type || operation_data || nonce)
pub fn create_signature_message(
    smart_account: &Pubkey,
    operation_type: &[u8],
    operation_data: &[u8],
    nonce: u64,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"squads-v2");
    hasher.hash(smart_account.as_ref());
    hasher.hash(operation_type);
    hasher.hash(operation_data);
    hasher.hash(&nonce.to_le_bytes());

    hasher.result().to_bytes()
}

/// Create message for vote signing
pub fn create_vote_message(proposal_key: &Pubkey, vote: u8, transaction_index: u64) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"proposal_vote_v2");
    hasher.hash(proposal_key.as_ref());
    hasher.hash(&[vote]);
    hasher.hash(&transaction_index.to_le_bytes());

    hasher.result().to_bytes()
}
