use anchor_lang::prelude::*;

use crate::{
    errors::SmartAccountError,
    interface::consensus_trait::Consensus,
    state::{ClientDataJsonReconstructionParams, SignerType},
    utils::{split_instructions_sysvar, verify_external_signatures},
};

/// Verify a V2 signer (native or external) for the given expected message.
///
/// remaining_accounts layout:
/// - Optional instructions sysvar (if external signer verification is required)
/// - Native signer accounts (if signer is native)
/// - Instruction-specific accounts
///
/// Returns WebAuthn counter updates to persist on the consensus account.
pub fn verify_v2_signer_signature<C: Consensus>(
    consensus_account: &C,
    signer_key: Pubkey,
    remaining_accounts: &[AccountInfo],
    expected_message: &[u8],
    client_data_params: Option<&ClientDataJsonReconstructionParams>,
) -> Result<Vec<(Pubkey, u64)>> {
    let (instructions_sysvar_opt, native_accounts) = split_instructions_sysvar(remaining_accounts);

    let is_native_signer = native_accounts
        .iter()
        .any(|acc| acc.key == &signer_key && acc.is_signer);

    if is_native_signer {
        return Ok(vec![]);
    }

    let signer = consensus_account
        .is_signer_v2(signer_key)
        .ok_or(SmartAccountError::NotASigner)?;

    if matches!(signer.signer_type(), SignerType::Native) {
        return Err(SmartAccountError::MissingSignature.into());
    }

    let instructions_sysvar = instructions_sysvar_opt
        .ok_or(SmartAccountError::MissingPrecompileInstruction)?;

    let verification = verify_external_signatures(
        instructions_sysvar,
        &[signer.clone()],
        expected_message,
        Some(&[signer_key]),
        client_data_params,
    )?;

    Ok(verification.counter_updates)
}

/// Verify V2 signer and apply counter updates to the consensus account.
/// Returns the signer key for event logging.
pub fn verify_v2_context<C: Consensus>(
    consensus_account: &mut C,
    signer_key: Pubkey,
    remaining_accounts: &[AccountInfo],
    expected_message: &[u8],
    client_data_params: Option<&ClientDataJsonReconstructionParams>,
) -> Result<Pubkey> {
    let updates = verify_v2_signer_signature(
        consensus_account,
        signer_key,
        remaining_accounts,
        expected_message,
        client_data_params,
    )?;

    if !updates.is_empty() {
        consensus_account.apply_counter_updates(&updates)?;
    }

    Ok(signer_key)
}
