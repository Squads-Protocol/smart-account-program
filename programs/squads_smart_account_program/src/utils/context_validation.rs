use crate::{consensus::ConsensusAccount, consensus_trait::Consensus, errors::*, state::*};
use anchor_lang::prelude::*;

pub fn validate_synchronous_consensus(
    consensus_account: &ConsensusAccount,
    num_signers: u8,
    remaining_accounts: &[AccountInfo],
) -> Result<()> {
    // Settings must not be time locked
    require_eq!(consensus_account.time_lock(), 0, SmartAccountError::TimeLockNotZero);

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
        aggregated_permissions.mask == 7,
        SmartAccountError::InsufficientAggregatePermissions
    );

    // Verify threshold is met across all voting permissions
    require!(
        vote_permission_count >= consensus_account.threshold() as usize,
        SmartAccountError::InsufficientVotePermissions
    );

    Ok(())
}

pub fn validate_settings_actions(actions: &Vec<SettingsAction>) -> Result<()> {
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
