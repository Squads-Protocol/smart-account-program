use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    ed25519_program,
    instruction::Instruction,
    pubkey,
    secp256k1_program,
    sysvar::instructions::load_instruction_at_checked,
};

use crate::errors::SmartAccountError;
use super::super::SignerType;

pub const SECP256R1_PROGRAM_ID: Pubkey = pubkey!("Secp256r1SigVerify1111111111111111111111111");

// ============================================================================
// Precompile Signature Parsing
// ============================================================================

#[derive(Debug)]
pub struct ParsedPrecompileSignature {
    pub signer_type: SignerType,
    pub public_key: Vec<u8>,
    pub message: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SignerMatchKey {
    P256([u8; 33]),
    Secp256k1([u8; 20]),
    Ed25519([u8; 32]),
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

/// Get the number of signatures in a precompile instruction's data.
///
/// Each precompile type encodes num_signatures differently:
/// - Ed25519/Secp256r1: 2-byte LE u16
/// - Secp256k1: 1-byte u8
pub fn get_precompile_num_signatures(
    precompile_data: &[u8],
    signer_type: SignerType,
) -> Result<usize> {
    match signer_type {
        SignerType::P256Webauthn | SignerType::P256Native => get_num_signatures::<Secp256r1>(precompile_data),
        SignerType::Secp256k1 => get_num_signatures::<LegacySecp256k1>(precompile_data),
        SignerType::Ed25519External => get_num_signatures::<Ed25519>(precompile_data),
        SignerType::Native => Err(SmartAccountError::InvalidSignerType.into()),
    }
}

/// Parse a precompile signature from instruction data
pub fn parse_precompile_signature(
    precompile_ix: &Instruction,
    instructions_sysvar: &AccountInfo,
    signer_type: SignerType,
    index: usize,
) -> Result<ParsedPrecompileSignature> {
    // Verify program ID matches expected precompile
    let expected_program_id = match signer_type {
        SignerType::P256Webauthn | SignerType::P256Native => SECP256R1_PROGRAM_ID,
        SignerType::Secp256k1 => secp256k1_program::ID,
        SignerType::Ed25519External => ed25519_program::ID,
        SignerType::Native => return Err(SmartAccountError::InvalidSignerType.into()),
    };
    require_keys_eq!(
        precompile_ix.program_id,
        expected_program_id,
        SmartAccountError::InvalidPrecompileProgram
    );

    let payload = match signer_type {
        SignerType::P256Webauthn | SignerType::P256Native => {
            extract_signature_payload::<Secp256r1>(&precompile_ix.data, instructions_sysvar, index)?
        }
        SignerType::Secp256k1 => {
            extract_signature_payload::<LegacySecp256k1>(&precompile_ix.data, instructions_sysvar, index)?
        }
        SignerType::Ed25519External => {
            extract_signature_payload::<Ed25519>(&precompile_ix.data, instructions_sysvar, index)?
        }
        SignerType::Native => return Err(SmartAccountError::InvalidSignerType.into()),
    };

    Ok(ParsedPrecompileSignature {
        signer_type,
        public_key: payload.public_key,
        message: payload.message,
        signature: payload.signature,
    })
}