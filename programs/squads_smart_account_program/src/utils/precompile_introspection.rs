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
use crate::state::{
    ClientDataJsonReconstructionParams, SignerType, SmartAccountSigner,
    WEBAUTHN_SIGNATURE_MIN_SIZE,
};

pub const SECP256R1_PROGRAM_ID: Pubkey = pubkey!("Secp256r1SigVerify1111111111111111111111111");

// ============================================================================
// WebAuthn Authenticator Data Parser
// Ported from external-signature-program
// ============================================================================

/// Wrapper for parsing WebAuthn authenticator data
pub struct AuthDataParser<'a> {
    auth_data: &'a [u8],
}

impl<'a> AuthDataParser<'a> {
    /// Creates a new AuthDataParser
    pub fn new(auth_data: &'a [u8]) -> Self {
        Self { auth_data }
    }

    /// Gets the RP ID hash (first 32 bytes)
    pub fn rp_id_hash(&self) -> &'a [u8] {
        &self.auth_data[0..32]
    }

    /// Checks if the user is present based on the flags
    pub fn is_user_present(&self) -> bool {
        self.auth_data[32] & 0x01 != 0
    }

    /// Checks if the user is verified based on the flags
    pub fn is_user_verified(&self) -> bool {
        self.auth_data[32] & 0x04 != 0
    }

    /// Gets the counter from the authenticator data (bytes 33-36, big-endian)
    pub fn get_counter(&self) -> u32 {
        u32::from_be_bytes([
            self.auth_data[33],
            self.auth_data[34],
            self.auth_data[35],
            self.auth_data[36],
        ])
    }
}

// ============================================================================
// ClientDataJSON Reconstruction
// Ported from external-signature-program
// ============================================================================

/// Base64URL alphabet for encoding
const BASE64URL_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Encode bytes as base64url (no padding)
fn base64url_encode(input: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity((input.len() * 4 + 2) / 3);

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        output.push(BASE64URL_ALPHABET[b0 >> 2]);
        output.push(BASE64URL_ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)]);

        if chunk.len() > 1 {
            output.push(BASE64URL_ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)]);
        }
        if chunk.len() > 2 {
            output.push(BASE64URL_ALPHABET[b2 & 0x3f]);
        }
    }

    output
}

/// Reconstruct clientDataJSON from compact parameters
///
/// This reconstructs the full clientDataJSON that was hashed to produce clientDataHash.
/// The format is:
/// {"type":"webauthn.get","challenge":"<base64url>","origin":"<origin>","crossOrigin":<bool>}
pub fn reconstruct_client_data_json(
    params: &ClientDataJsonReconstructionParams,
    rp_id: &[u8],
    challenge: &[u8],
) -> Vec<u8> {
    let mut json = Vec::with_capacity(256);

    // Start JSON object
    json.extend_from_slice(b"{\"type\":\"webauthn.");

    // Type: "create" or "get"
    if params.is_create() {
        json.extend_from_slice(b"create");
    } else {
        json.extend_from_slice(b"get");
    }

    // Challenge (base64url encoded)
    json.extend_from_slice(b"\",\"challenge\":\"");
    let encoded_challenge = base64url_encode(challenge);
    json.extend_from_slice(&encoded_challenge);

    // Origin
    json.extend_from_slice(b"\",\"origin\":\"");
    if params.is_http() {
        json.extend_from_slice(b"http://");
    } else {
        json.extend_from_slice(b"https://");
    }
    json.extend_from_slice(rp_id);

    // Optional port
    if let Some(port) = params.get_port() {
        json.push(b':');
        // Convert port to string bytes
        let port_str = port.to_string();
        json.extend_from_slice(port_str.as_bytes());
    }

    // Cross-origin
    json.extend_from_slice(b"\",\"crossOrigin\":");
    if params.is_cross_origin() {
        json.extend_from_slice(b"true");
    } else {
        json.extend_from_slice(b"false");
    }

    // Google extra field (some authenticators add this)
    if params.has_google_extra() {
        json.extend_from_slice(b",\"androidPackageName\":\"com.google.android.gms\"");
    }

    // Close JSON object
    json.push(b'}');

    json
}

#[derive(Debug)]
pub struct ParsedPrecompileSignature {
    pub signer_type: SignerType,
    pub public_key: Vec<u8>,
    pub message: Vec<u8>,
    pub signature: Vec<u8>,
}

pub struct ExternalSignatureVerification {
    pub verified_key_ids: Vec<Pubkey>,
    pub verified_count: usize,
    /// Counter updates for WebAuthn signers (key_id -> new_counter)
    pub counter_updates: Vec<(Pubkey, u64)>,
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
    signer_type: SignerType,
    index: usize,
) -> Result<ParsedPrecompileSignature> {
    // Verify program ID matches expected precompile
    let expected_program_id = match signer_type {
        SignerType::P256Webauthn => SECP256R1_PROGRAM_ID,
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
        SignerType::P256Webauthn => {
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

/// Verify external signatures from precompile instructions in the transaction
///
/// For WebAuthn signers, `client_data_params` must be provided to reconstruct
/// the clientDataJSON for verification.
pub fn verify_external_signatures(
    instructions_sysvar: &AccountInfo,
    external_signers: &[SmartAccountSigner],
    expected_message: &[u8],
    required_key_ids: Option<&[Pubkey]>,
    client_data_params: Option<&ClientDataJsonReconstructionParams>,
) -> Result<ExternalSignatureVerification> {
    let current_idx = load_current_index_checked(instructions_sysvar)
        .map_err(|_| SmartAccountError::MissingPrecompileInstruction)? as usize;

    let mut verified_key_ids = Vec::new();
    let mut used_signatures: HashSet<Vec<u8>> = HashSet::new();
    let mut counter_updates: Vec<(Pubkey, u64)> = Vec::new();

    for ix_idx in 0..current_idx {
        let ix = load_instruction_at_checked(ix_idx, instructions_sysvar)
            .map_err(|_| SmartAccountError::MissingPrecompileInstruction)?;

        let signer_type = match &ix.program_id {
            id if id == &SECP256R1_PROGRAM_ID => SignerType::P256Webauthn,
            id if id == &secp256k1_program::ID => SignerType::Secp256k1,
            id if id == &ed25519_program::ID => SignerType::Ed25519External,
            _ => continue,
        };

        // Get number of signatures in this precompile instruction
        let num = match signer_type {
            SignerType::P256Webauthn => get_num_signatures::<Secp256r1>(&ix.data)?,
            SignerType::Secp256k1 => get_num_signatures::<LegacySecp256k1>(&ix.data)?,
            SignerType::Ed25519External => get_num_signatures::<Ed25519>(&ix.data)?,
            SignerType::Native => 0,
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
                        let verification = verify_signed_message(&parsed.message, expected_message, signer, client_data_params)?;
                        if !verified_key_ids.contains(&signer.key()) {
                            verified_key_ids.push(signer.key());
                            // Track counter updates for WebAuthn signers
                            if let Some(new_counter) = verification.new_counter {
                                counter_updates.push((signer.key(), new_counter));
                            }
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
        counter_updates,
    })
}

fn pubkeys_match(stored: &[u8], parsed: &[u8], signer_type: SignerType) -> bool {
    match signer_type {
        SignerType::P256Webauthn => stored == parsed,
        SignerType::Secp256k1 => {
            // For secp256k1, stored is uncompressed pubkey (64 bytes), parsed is eth_address (20 bytes)
            if stored.len() != 64 || parsed.len() != 20 {
                return false;
            }
            compute_eth_address(stored).as_slice() == parsed
        }
        SignerType::Ed25519External => stored == parsed,
        SignerType::Native => false,
    }
}

fn compute_eth_address(pubkey: &[u8]) -> [u8; 20] {
    use anchor_lang::solana_program::keccak::hash as keccak256;

    let hash = keccak256(pubkey);
    let mut eth_address = [0u8; 20];
    eth_address.copy_from_slice(&hash.0[12..32]);
    eth_address
}

/// Result of verifying a signed message, includes optional counter update for WebAuthn
struct SignedMessageVerification {
    /// New counter value for WebAuthn signers (None for other signer types)
    new_counter: Option<u64>,
}

/// Verify a signed message using the correct WebAuthn flow
///
/// For WebAuthn/P256 signers:
/// - The signed message format is: authenticatorData || clientDataHash
/// - We split at (len - 32) to get auth_data and client_data_hash
/// - We reconstruct the expected clientDataJSON and hash it
/// - We compare the hashes to verify the challenge matches
///
/// For other external signers (Ed25519, Secp256k1):
/// - Direct message comparison
fn verify_signed_message(
    signed: &[u8],
    expected_challenge: &[u8],
    signer: &SmartAccountSigner,
    client_data_params: Option<&ClientDataJsonReconstructionParams>,
) -> Result<SignedMessageVerification> {
    match signer {
        SmartAccountSigner::P256Webauthn { data, .. } => {
            // Minimum size: authenticatorData (37 min) + clientDataHash (32) = 69 bytes
            require!(signed.len() >= WEBAUTHN_SIGNATURE_MIN_SIZE, SmartAccountError::InvalidPrecompileData);

            // Split the message: authenticatorData || clientDataHash
            // clientDataHash is always the last 32 bytes
            let (auth_data, client_data_hash) = signed.split_at(signed.len() - 32);

            // Parse authenticator data using AuthDataParser
            let auth_parser = AuthDataParser::new(auth_data);

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

            // Reconstruct clientDataJSON and verify hash
            // We need the reconstruction params to know origin details
            let params = client_data_params
                .ok_or(SmartAccountError::MissingClientDataParams)?;

            let rp_id = data.get_rp_id();
            let reconstructed_client_data = reconstruct_client_data_json(params, rp_id, expected_challenge);

            // Hash the reconstructed clientDataJSON
            use anchor_lang::solana_program::hash::hash;
            let reconstructed_hash = hash(&reconstructed_client_data);

            // Compare hashes
            require!(
                client_data_hash == reconstructed_hash.to_bytes().as_slice(),
                SmartAccountError::PrecompileMessageMismatch
            );

            // Always return the counter value for persistence.
            // This ensures that:
            // 1. Counter 0 can be marked as "used" after first authentication
            // 2. Any higher counter gets persisted
            // The caller MUST persist this value to prevent replay attacks.
            Ok(SignedMessageVerification { new_counter: Some(sign_counter) })
        }
        _ => {
            // For non-WebAuthn signers, direct comparison
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

/// Create message for proposal creation signing
///
/// Format: hash("proposal_create_v2" || consensus_account_key || transaction_index || draft)
pub fn create_proposal_create_message(
    consensus_account_key: &Pubkey,
    transaction_index: u64,
    draft: bool,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"proposal_create_v2");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());
    hasher.hash(&[draft as u8]);

    hasher.result().to_bytes()
}

/// Create message for proposal activation signing
///
/// Format: hash("proposal_activate_v2" || proposal_key || transaction_index)
pub fn create_proposal_activate_message(
    proposal_key: &Pubkey,
    transaction_index: u64,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"proposal_activate_v2");
    hasher.hash(proposal_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    // TODO: if it's a passKey we need to have the additional data as well.

    // TODO: add nonce + 1.

    hasher.result().to_bytes()
}

/// Create message for async transaction execution signing
///
/// Format: hash("transaction_execute_v2" || transaction_key || transaction_index)
pub fn create_execute_transaction_message(
    transaction_key: &Pubkey,
    transaction_index: u64,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"transaction_execute_v2");
    hasher.hash(transaction_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    hasher.result().to_bytes()
}

/// Create message for async settings transaction execution signing
///
/// Format: hash("settings_tx_execute_v2" || transaction_key || transaction_index)
pub fn create_execute_settings_transaction_message(
    transaction_key: &Pubkey,
    transaction_index: u64,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"settings_tx_execute_v2");
    hasher.hash(transaction_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    hasher.result().to_bytes()
}

pub fn create_increment_account_index_message(
    settings: &Pubkey,
    signer_key: Pubkey,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"increment_account_index_v2");
    hasher.hash(settings.as_ref());
    hasher.hash(signer_key.as_ref());

    hasher.result().to_bytes()
}

/// Create message for batch creation signing
///
/// Format: hash("batch_create_v2" || settings_key || creator_key || account_index)
pub fn create_batch_create_message(
    settings_key: &Pubkey,
    creator_key: Pubkey,
    account_index: u8,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"batch_create_v2");
    hasher.hash(settings_key.as_ref());
    hasher.hash(creator_key.as_ref());
    hasher.hash(&[account_index]);

    hasher.result().to_bytes()
}

/// Create message for adding a transaction to a batch
///
/// Format: hash("batch_add_tx_v2" || batch_key || signer_key || transaction_index)
pub fn create_batch_add_transaction_message(
    batch_key: &Pubkey,
    signer_key: Pubkey,
    transaction_index: u64,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"batch_add_tx_v2");
    hasher.hash(batch_key.as_ref());
    hasher.hash(signer_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    hasher.result().to_bytes()
}

/// Create message for executing a transaction from a batch
///
/// Format: hash("batch_execute_tx_v2" || batch_key || signer_key || transaction_index)
pub fn create_batch_execute_transaction_message(
    batch_key: &Pubkey,
    signer_key: Pubkey,
    transaction_index: u64,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"batch_execute_tx_v2");
    hasher.hash(batch_key.as_ref());
    hasher.hash(signer_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    hasher.result().to_bytes()
}

/// Create message for transaction buffer creation signing
///
/// Format: hash("tx_buffer_create_v2" || consensus_account_key || creator_key || buffer_index || account_index || final_hash || final_size)
pub fn create_transaction_buffer_create_message(
    consensus_account_key: &Pubkey,
    creator_key: &Pubkey,
    buffer_index: u8,
    account_index: u8,
    final_buffer_hash: &[u8; 32],
    final_buffer_size: u16,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"tx_buffer_create_v2");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(creator_key.as_ref());
    hasher.hash(&[buffer_index]);
    hasher.hash(&[account_index]);
    hasher.hash(final_buffer_hash);
    hasher.hash(&final_buffer_size.to_le_bytes());

    hasher.result().to_bytes()
}

/// Create message for transaction buffer extension signing
///
/// Format: hash("tx_buffer_extend_v2" || buffer_key || chunk_hash)
pub fn create_transaction_buffer_extend_message(
    buffer_key: &Pubkey,
    buffer_chunk: &[u8],
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::{hash, Hasher};

    let chunk_hash = hash(buffer_chunk);
    let mut hasher = Hasher::default();
    hasher.hash(b"tx_buffer_extend_v2");
    hasher.hash(buffer_key.as_ref());
    hasher.hash(chunk_hash.as_ref());

    hasher.result().to_bytes()
}

/// Create message for transaction creation from buffer signing
///
/// Format: hash("tx_from_buffer_v2" || buffer_key || transaction_index)
pub fn create_transaction_from_buffer_message(
    buffer_key: &Pubkey,
    transaction_index: u64,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"tx_from_buffer_v2");
    hasher.hash(buffer_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    hasher.result().to_bytes()
}

/// Create message for synchronous consensus verification
/// 
/// This is used by external signers to prove they're authorizing a sync transaction.
/// Format: hash("squads-sync" || consensus_account_key || transaction_index)
pub fn create_sync_consensus_message(
    consensus_account_key: &Pubkey,
    transaction_index: u64,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"squads-sync");
    hasher.hash(consensus_account_key.as_ref());
    // Note: We use transaction_index as a nonce to prevent replay
    hasher.hash(&transaction_index.to_le_bytes());

    hasher.result().to_bytes()
}

/// Create message for async transaction creation signing
///
/// Format: hash("transaction_create_v2" || consensus_account_key || transaction_index)
pub fn create_transaction_message(
    consensus_account_key: &Pubkey,
    transaction_index: u64,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"transaction_create_v2");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    hasher.result().to_bytes()
}

/// Create message for settings transaction creation signing
///
/// Format: hash("settings_tx_create_v2" || settings_key || transaction_index)
pub fn create_settings_transaction_create_message(
    settings_key: &Pubkey,
    transaction_index: u64,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::Hasher;

    let mut hasher = Hasher::default();
    hasher.hash(b"settings_tx_create_v2");
    hasher.hash(settings_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    hasher.result().to_bytes()
}
