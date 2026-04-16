use crate::{consensus::ConsensusAccount, consensus_trait::Consensus, errors::*, state::*};
use crate::state::signer_v2::precompile::verify_precompile_signers;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::SignerType;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::Hasher;

/// # Sync vs Async Verification Architecture
///
/// ## Sync Path (`validate_synchronous_consensus`)
/// All signers verified in one instruction. `remaining_accounts` layout:
/// ```text
/// [native_signers..., external_signers..., instructions_sysvar?]
/// ```
/// - Native signers: `AccountInfo.is_signer = true`, verified by break-on-false loop
/// - External signers: `AccountInfo.is_signer = false`, split by EVD into precompile/syscall slices
/// - Instructions sysvar (optional): at index `num_signer`, required if precompile signers present
/// - `extra_verification_data`: typed enum slice, one entry per external signer
///   - Precompile entries (`is_precompile()`) must be first, matching packed sig order in ix[0]
///   - Syscall entries follow
///
/// ## Async Path (`Consensus::verify_signer`)
/// One signer per instruction. `remaining_accounts` layout:
/// ```text
/// [instructions_sysvar?, ...other_accounts]
/// ```
/// - Instructions sysvar (optional): first account if present, detected by `split_instructions_sysvar()`
/// - Single signer provided in instruction accounts (not `remaining_accounts`)
/// - `extra_verification_data`: applies to the single signer being verified

/// Validate synchronous consensus for all signer types (native, session keys, external).
///
/// # Verification Strategy
/// 1. Native signers: loop `is_signer=true`, break on first `false`
/// 2. External signers (EVD-driven):
///    - `take_while(is_precompile).count()` → split EVD into precompile/syscall slices
///    - Precompile: batch verify via `verify_precompile_signers` (single ix load, direct index)
///    - Syscall: per-signer `verify_external_signer_via_syscall`
/// 3. Threshold + permission checks
///
/// # Arguments
/// * `num_signer` - Total count of ALL signers (native + external) in remaining_accounts
///   - Native signers have AccountInfo.is_signer = true
///   - External signers have AccountInfo.is_signer = false
///   - Instructions sysvar must be at position num_signer
pub fn validate_synchronous_consensus(
    consensus_account: &mut ConsensusAccount,
    num_signer: u8,
    remaining_accounts: &[AccountInfo],
    message: Hasher,
    extra_verification_data: &[ExtraVerificationData],
) -> Result<()> {
    // Time lock must be 0 for sync transactions
    require_eq!(
        consensus_account.time_lock(),
        0,
        SmartAccountError::TimeLockNotZero
    );

    // Validate num_signer doesn't exceed remaining_accounts length
    require!(
        (num_signer as usize) <= remaining_accounts.len(),
        SmartAccountError::InvalidSignerCount
    );

    // Initialize state
    let mut verified_keys = Vec::with_capacity(num_signer as usize);
    let mut aggregated_permissions = Permissions { mask: 0 };
    let mut vote_permission_count = 0;
    let now = Clock::get()?.unix_timestamp as u64;

    let mut counter_updates = Vec::new();
    let mut nonce_updates = Vec::new();

    // ============================================================
    // PHASE 1: Process native signers (is_signer=true)
    // ============================================================
    let mut num_native_signers = 0u8;
    for account_info in &remaining_accounts[..num_signer as usize] {
        // Stop when we hit external signers (is_signer=false)
        if !account_info.is_signer {
            break;
        }

        let signer_key = *account_info.key;

        // Direct lookup: check if it's a direct signer or session key
        let member = consensus_account
            .is_signer_v2(signer_key)
            .or_else(|| consensus_account.find_signer_by_session_key(signer_key, now))
            .ok_or(SmartAccountError::NotASigner)?;

        // Use canonical signer key (parent key for session keys)
        // This prevents double-counting when someone signs with both
        // a session key and the parent external signer
        let resolved_key = member.key();
        require!(
            !verified_keys.contains(&resolved_key),
            SmartAccountError::DuplicateSigner
        );
        verified_keys.push(resolved_key);

        // Aggregate permissions
        let permissions = member.permissions();
        aggregated_permissions.mask |= permissions.mask;
        if permissions.has(Permission::Vote) {
            vote_permission_count += 1;
        }

        num_native_signers += 1;
    }

    // ============================================================
    // PHASE 2: EVD-driven external signer verification
    //
    // EVD is the source of truth for verification method.
    // take_while(is_precompile) splits into two slices:
    //   - Precompile: batch verified (single ix load, direct index)
    //   - Syscall: per-signer verification
    // ============================================================
    let num_external_signers = num_signer
        .checked_sub(num_native_signers)
        .ok_or(SmartAccountError::Overflow)?;

    if num_external_signers > 0 {
        require!(
            extra_verification_data.len() == num_external_signers as usize,
            SmartAccountError::InvalidPayload
        );

        let external_signers = &remaining_accounts[num_native_signers as usize..num_signer as usize];
        let num_precompile = extra_verification_data.iter()
            .take_while(|e| e.is_precompile())
            .count();

        // Validate EVD ordering: precompile entries must be first, then syscall entries only
        require!(
            extra_verification_data[num_precompile..].iter().all(|e| e.is_syscall()),
            SmartAccountError::InvalidPayload
        );

        // Resolve all external signers upfront
        let members: Vec<SmartAccountSigner> = external_signers.iter()
            .map(|acc| consensus_account
                .is_signer_v2(*acc.key)
                .ok_or_else(|| error!(SmartAccountError::NotASigner)))
            .collect::<Result<_>>()?;

        // --- PRECOMPILE: batch verify, single ix load, direct index ---
        if num_precompile > 0 {
            let sysvar = remaining_accounts
                .get(num_signer as usize)
                .filter(|acc| acc.key == &anchor_lang::solana_program::sysvar::instructions::ID)
                .ok_or(SmartAccountError::MissingPrecompileInstruction)?;

            let precompile_results = verify_precompile_signers(
                sysvar,
                &members[..num_precompile],
                &extra_verification_data[..num_precompile],
                &message,
            )?;

            for (member, (counter_opt, next_nonce)) in
                members[..num_precompile].iter().zip(precompile_results)
            {
                let resolved_key = member.key();
                if let Some(counter) = counter_opt {
                    counter_updates.push((resolved_key, counter));
                }
                nonce_updates.push((resolved_key, next_nonce));

                require!(
                    !verified_keys.contains(&resolved_key),
                    SmartAccountError::DuplicateSigner
                );
                verified_keys.push(resolved_key);

                let permissions = member.permissions();
                aggregated_permissions.mask |= permissions.mask;
                if permissions.has(Permission::Vote) {
                    vote_permission_count += 1;
                }
            }
        }

        // --- SYSCALL: per-signer, remaining slice ---
        for (member, evd) in members[num_precompile..].iter()
            .zip(extra_verification_data[num_precompile..].iter())
        {
            require!(
                member.signer_type() != SignerType::P256Webauthn
                    && member.signer_type() != SignerType::P256Native,
                SmartAccountError::PrecompileRequired
            );

            let (counter_opt, next_nonce) = verify_external_signer_via_syscall(
                member, &message, evd,
            )?;

            let resolved_key = member.key();
            if let Some(counter) = counter_opt {
                counter_updates.push((resolved_key, counter));
            }
            nonce_updates.push((resolved_key, next_nonce));

            require!(
                !verified_keys.contains(&resolved_key),
                SmartAccountError::DuplicateSigner
            );
            verified_keys.push(resolved_key);

            let permissions = member.permissions();
            aggregated_permissions.mask |= permissions.mask;
            if permissions.has(Permission::Vote) {
                vote_permission_count += 1;
            }
        }
    }

    // Apply state updates
    if !counter_updates.is_empty() {
        consensus_account.apply_counter_updates(&counter_updates)?;
    }
    for (key, nonce) in nonce_updates {
        consensus_account.apply_nonce_update(&key, nonce)?;
    }

    // Final validation
    let threshold = consensus_account.threshold() as usize;
    require!(
        verified_keys.len() >= threshold,
        SmartAccountError::InvalidSignerCount
    );
    require!(
        aggregated_permissions.mask == Permissions::all().mask,
        SmartAccountError::InsufficientAggregatePermissions
    );
    require!(
        vote_permission_count >= threshold,
        SmartAccountError::InsufficientVotePermissions
    );

    Ok(())
}

/// Verify external signer via syscalls (fallback when no precompile instruction).
///
/// # Arguments
/// * `signer` - The external signer to verify
/// * `message` - The base message hasher (nonce will be appended)
/// * `extra_verification_data` - Typed verification data containing the signature
pub fn verify_external_signer_via_syscall(
    signer: &SmartAccountSigner,
    message: &Hasher,
    extra_verification_data: &ExtraVerificationData,
) -> Result<(Option<u64>, u64)> {
    // Get current nonce
    let nonce = signer
        .nonce()
        .ok_or_else(|| error!(SmartAccountError::InvalidSignerType))?;
    let next_nonce = nonce
        .checked_add(1)
        .ok_or_else(|| error!(SmartAccountError::NonceExhausted))?;

    // Hash message with nonce
    let mut message_with_nonce = message.clone();
    message_with_nonce.hash(&next_nonce.to_le_bytes());
    let expected_message = message_with_nonce.result().to_bytes();

    #[cfg(feature = "testing")]
    msg!("syscall_verify: nonce={}, next_nonce={}, msg_hash={:?}", nonce, next_nonce, &expected_message[..8]);

    match (signer, extra_verification_data) {
        (SmartAccountSigner::Ed25519External { data, .. }, ExtraVerificationData::Ed25519Syscall { signature }) => {
            #[cfg(feature = "testing")]
            msg!("ed25519_syscall: pubkey={:?}, sig_len={}", &data.external_pubkey[..8], signature.len());

            crate::state::signer_v2::ed25519_syscall::ed25519_syscall_verify(
                &data.external_pubkey,
                signature,
                &expected_message,
            )
            .map_err(|_| SmartAccountError::InvalidSignature)?;

            Ok((None, next_nonce))
        }

        (SmartAccountSigner::Secp256k1 { data, .. }, ExtraVerificationData::Secp256k1Syscall { signature, recovery_id }) => {
            // eth_address must be derived (has_eth_address=true) for syscall verification
            require!(data.has_eth_address, SmartAccountError::InvalidSignerType);

            crate::state::signer_v2::secp256k1_syscall::secp256k1_syscall_verify(
                &data.eth_address,
                signature,
                *recovery_id,
                &expected_message,
            )
            .map_err(|_| SmartAccountError::InvalidSignature)?;

            Ok((None, next_nonce))
        }

        (SmartAccountSigner::P256Webauthn { .. }, _)
        | (SmartAccountSigner::P256Native { .. }, _) => {
            Err(SmartAccountError::PrecompileRequired.into())
        }

        _ => Err(SmartAccountError::InvalidSignerType.into()),
    }
}

pub fn validate_settings_actions(actions: &[SettingsAction]) -> Result<()> {
    // Config transaction must have at least one action
    require!(!actions.is_empty(), SmartAccountError::NoActions);

    let current_timestamp = Clock::get()?.unix_timestamp;
    // time_lock must not exceed the maximum allowed.
    for action in actions {
        if let SettingsAction::SetTimeLock { new_time_lock, .. } = action {
            require!(
                *new_time_lock <= MAX_TIME_LOCK,
                SmartAccountError::TimeLockExceedsMaxAllowed
            );
        }
        // Expiration must be greater than the current timestamp.
        if let SettingsAction::AddSpendingLimit { expiration, .. } = action {
            if *expiration != i64::MAX {
                require!(
                    *expiration > current_timestamp,
                    SmartAccountError::SpendingLimitExpired
                );
            }
        }
    }

    Ok(())
}
