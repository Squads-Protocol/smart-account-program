use crate::{consensus::ConsensusAccount, consensus_trait::Consensus, errors::*, state::*};
use anchor_lang::prelude::*;

pub fn validate_synchronous_consensus(
    consensus_account: &ConsensusAccount,
    num_signers: u8,
    remaining_accounts: &[AccountInfo],
) -> Result<()> {
    // Settings must not be time locked
    require_eq!(
        consensus_account.time_lock(),
        0,
        SmartAccountError::TimeLockNotZero
    );

    // Get signers from remaining accounts using threshold
    let required_signer_count = consensus_account.threshold() as usize;
    let signer_count = num_signers as usize;
    require!(
        signer_count >= required_signer_count,
        SmartAccountError::InvalidSignerCount
    );

    let signers = remaining_accounts
        .get(..signer_count)
        .ok_or(SmartAccountError::InvalidSignerCount)?;

    // Setup the aggregated permissions and the vote permission count
    let mut aggregated_permissions = Permissions { mask: 0 };
    let mut vote_permission_count = 0;
    let mut seen_signers = Vec::with_capacity(signer_count);

    // Check permissions for all signers
    for signer in signers.iter() {
        if let Some(member_index) = consensus_account.is_signer(signer.key()) {
            // Check that the signer is indeed a signer
            if !signer.is_signer {
                return err!(SmartAccountError::MissingSignature);
            }
            // Check for duplicate signer
            if seen_signers.contains(&signer.key()) {
                return err!(SmartAccountError::DuplicateSigner);
            }
            seen_signers.push(signer.key());

            let signer_permissions = consensus_account.signers()[member_index].permissions;
            // Add to the aggregated permissions mask
            aggregated_permissions.mask |= signer_permissions.mask;

            // Count the vote permissions
            if signer_permissions.has(Permission::Vote) {
                vote_permission_count += 1;
            }
        } else {
            return err!(SmartAccountError::NotASigner);
        }
    }

    // Check if we have all required permissions (Initiate | Vote | Execute = 7)
    require!(
        aggregated_permissions.mask == Permissions::all().mask,
        SmartAccountError::InsufficientAggregatePermissions
    );

    // Verify threshold is met across all voting permissions
    require!(
        vote_permission_count >= consensus_account.threshold() as usize,
        SmartAccountError::InsufficientVotePermissions
    );

    Ok(())
}

/// Helper function to count voters from a slice of signers
fn count_voters(signers: &[SmartAccountSigner]) -> usize {
    signers
        .iter()
        .filter(|s| s.permissions.has(Permission::Vote))
        .count()
}

/// Helper function to check for duplicate signers
fn has_duplicate_signers(signers: &[SmartAccountSigner]) -> bool {
    let mut seen = Vec::with_capacity(signers.len());
    for signer in signers {
        if seen.contains(&signer.key) {
            return true;
        }
        seen.push(signer.key);
    }
    false
}

/// Helper function to check for duplicate pubkeys
fn has_duplicate_pubkeys(pubkeys: &[Pubkey]) -> bool {
    let mut seen = Vec::with_capacity(pubkeys.len());
    for pubkey in pubkeys {
        if seen.contains(pubkey) {
            return true;
        }
        seen.push(*pubkey);
    }
    false
}

/// Validate policy signers, threshold, and time_lock
fn validate_policy_governance(
    signers: &[SmartAccountSigner],
    threshold: u16,
    time_lock: u32,
) -> Result<()> {
    // Signers must not be empty
    require!(!signers.is_empty(), SmartAccountError::EmptySigners);

    // No duplicate signers
    require!(
        !has_duplicate_signers(signers),
        SmartAccountError::DuplicateSigner
    );

    // All signers must have valid permissions (mask < 8)
    require!(
        signers.iter().all(|s| s.permissions.mask < 8),
        SmartAccountError::UnknownPermission
    );

    // Threshold must be greater than 0
    require!(threshold > 0, SmartAccountError::InvalidThreshold);

    // Threshold must not exceed the number of voters
    let num_voters = count_voters(signers);
    require!(
        usize::from(threshold) <= num_voters,
        SmartAccountError::InvalidThreshold
    );

    // Time lock must not exceed maximum
    require!(
        time_lock <= MAX_TIME_LOCK,
        SmartAccountError::TimeLockExceedsMaxAllowed
    );

    Ok(())
}

pub fn validate_settings_actions(actions: &Vec<SettingsAction>) -> Result<()> {
    // Config transaction must have at least one action
    require!(!actions.is_empty(), SmartAccountError::NoActions);

    let current_timestamp = Clock::get()?.unix_timestamp;

    for action in actions {
        match action {
            // AddSigner: validate permissions mask
            SettingsAction::AddSigner { new_signer } => {
                require!(
                    new_signer.permissions.mask < 8,
                    SmartAccountError::UnknownPermission
                );
            }

            // ChangeThreshold: threshold must be > 0
            SettingsAction::ChangeThreshold { new_threshold } => {
                require!(*new_threshold > 0, SmartAccountError::InvalidThreshold);
            }

            // SetTimeLock: must not exceed maximum
            SettingsAction::SetTimeLock { new_time_lock } => {
                require!(
                    *new_time_lock <= MAX_TIME_LOCK,
                    SmartAccountError::TimeLockExceedsMaxAllowed
                );
            }

            // AddSpendingLimit: validate amount, signers, and expiration
            SettingsAction::AddSpendingLimit {
                amount,
                signers,
                expiration,
                ..
            } => {
                // Amount must be non-zero
                require!(
                    *amount != 0,
                    SmartAccountError::SpendingLimitInvalidAmount
                );

                // Signers must not be empty
                require!(!signers.is_empty(), SmartAccountError::EmptySigners);

                // No duplicate signers
                require!(
                    !has_duplicate_pubkeys(signers),
                    SmartAccountError::DuplicateSigner
                );

                // Expiration must be greater than the current timestamp (unless i64::MAX)
                if *expiration != i64::MAX {
                    require!(
                        *expiration > current_timestamp,
                        SmartAccountError::SpendingLimitExpired
                    );
                }
            }

            // SetArchivalAuthority: reject as it's not implemented and will always fail
            SettingsAction::SetArchivalAuthority { .. } => {
                return err!(SmartAccountError::NotImplemented);
            }

            // PolicyCreate: validate governance params and payload
            SettingsAction::PolicyCreate {
                signers,
                threshold,
                time_lock,
                policy_creation_payload,
                start_timestamp,
                expiration_args,
                ..
            } => {
                // Validate governance parameters
                validate_policy_governance(signers, *threshold, *time_lock)?;

                // Validate the policy creation payload by calling to_policy_state
                // This catches invalid configurations early
                policy_creation_payload.to_policy_state()?;

                // If expiration is Timestamp, check it's greater than start
                if let Some(PolicyExpirationArgs::Timestamp(exp_timestamp)) = expiration_args {
                    let start = start_timestamp.unwrap_or(current_timestamp);
                    require!(
                        *exp_timestamp > start,
                        SmartAccountError::SpendingLimitInvariantExpirationSmallerThanStart
                    );
                }
            }

            // PolicyUpdate: validate governance params and payload
            SettingsAction::PolicyUpdate {
                signers,
                threshold,
                time_lock,
                policy_update_payload,
                expiration_args,
                ..
            } => {
                // Validate governance parameters
                validate_policy_governance(signers, *threshold, *time_lock)?;

                // Validate the policy update payload by calling to_policy_state
                policy_update_payload.to_policy_state()?;

                // If expiration is Timestamp, check it's greater than current time
                if let Some(PolicyExpirationArgs::Timestamp(exp_timestamp)) = expiration_args {
                    require!(
                        *exp_timestamp > current_timestamp,
                        SmartAccountError::SpendingLimitInvariantExpirationSmallerThanStart
                    );
                }
            }

            // RemoveSigner, RemoveSpendingLimit, PolicyRemove: these require account state
            // to validate properly, so we keep runtime checks only
            SettingsAction::RemoveSigner { .. }
            | SettingsAction::RemoveSpendingLimit { .. }
            | SettingsAction::PolicyRemove { .. } => {
                // These actions require access to account state for validation,
                // so they are validated at execution time
            }
        }
    }

    Ok(())
}
