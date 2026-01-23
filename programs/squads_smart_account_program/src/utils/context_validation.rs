use crate::{consensus::ConsensusAccount, consensus_trait::Consensus, errors::*, state::*};
use anchor_lang::prelude::*;

use super::precompile_introspection::{
    create_sync_consensus_message, split_instructions_sysvar, verify_external_signatures,
};

/// Arguments for V2 synchronous consensus validation
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SyncConsensusV2Args {
    /// Number of native signers (directly signing the transaction)
    pub num_native_signers: u8,
    /// Key IDs of external signers (verified via precompile)
    pub external_signer_key_ids: Vec<Pubkey>,
    /// Client data params for WebAuthn verification
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
}

/// Result of V2 synchronous consensus validation
pub struct SyncConsensusV2Result {
    /// Number of accounts consumed for consensus (instructions_sysvar + native signers)
    pub accounts_consumed: usize,
    /// Counter updates for WebAuthn signers (key_id -> new_counter).
    /// The caller is responsible for applying these to the account after verification.
    pub counter_updates: Vec<(Pubkey, u64)>,
}

/// V2 synchronous consensus validation supporting both native and external signers.
///
/// remaining_accounts layout:
/// - First: instructions sysvar (if external signers used)
/// - Next `num_native_signers`: native signer accounts
/// - Rest: instruction-specific accounts
///
/// Returns the validation result including accounts consumed and any counter updates.
/// The caller is responsible for persisting counter updates to the Settings/Policy account.
pub fn validate_synchronous_consensus_v2(
    consensus_account: &ConsensusAccount,
    args: &SyncConsensusV2Args,
    consensus_account_key: Pubkey,
    remaining_accounts: &[AccountInfo],
) -> Result<SyncConsensusV2Result> {
    let current_timestamp = Clock::get()?.unix_timestamp as u64;
    validate_synchronous_consensus_v2_with_timestamp(
        consensus_account,
        args,
        consensus_account_key,
        remaining_accounts,
        current_timestamp,
    )
}

fn validate_synchronous_consensus_v2_with_timestamp(
    consensus_account: &ConsensusAccount,
    args: &SyncConsensusV2Args,
    consensus_account_key: Pubkey,
    remaining_accounts: &[AccountInfo],
    current_timestamp: u64,
) -> Result<SyncConsensusV2Result> {
    // Settings must not be time locked
    require_eq!(
        consensus_account.time_lock(),
        0,
        SmartAccountError::TimeLockNotZero
    );

    // Split off the instructions sysvar if present
    let (instructions_sysvar, accounts_after_sysvar) = split_instructions_sysvar(remaining_accounts);

    let native_signer_count = args.num_native_signers as usize;
    let external_signer_count = args.external_signer_key_ids.len();
    let total_signers = native_signer_count + external_signer_count;

    // Must meet threshold
    let required_signer_count = consensus_account.threshold() as usize;
    require!(
        total_signers >= required_signer_count,
        SmartAccountError::InvalidSignerCount
    );

    // Get native signer accounts from remaining_accounts (after instructions sysvar)
    let native_signers = accounts_after_sysvar
        .get(..native_signer_count)
        .ok_or(SmartAccountError::InvalidSignerCount)?;

    // Setup aggregated permissions and vote count
    let mut aggregated_permissions = Permissions { mask: 0 };
    let mut vote_permission_count = 0;
    let mut seen_signers: Vec<Pubkey> = Vec::with_capacity(total_signers);
    let mut seen_native_keys: Vec<Pubkey> = Vec::new();
    let mut seen_session_keys: Vec<Pubkey> = Vec::new();
    let mut counter_updates: Vec<(Pubkey, u64)> = Vec::new();

    // Validate native signers (including session key authentication)
    for signer in native_signers.iter() {
        // Check that the signer is indeed signing
        if !signer.is_signer {
            return err!(SmartAccountError::MissingSignature);
        }

        // First, check if the signer is a direct native signer
        let signer_key = signer.key();
        if seen_session_keys.contains(&signer_key) {
            return err!(SmartAccountError::DuplicateSessionKey);
        }
        seen_session_keys.push(signer_key);

        if let Some(member) = consensus_account.is_signer_v2(signer_key) {
            // Check for duplicate signer
            if seen_signers.contains(&signer_key) {
                return err!(SmartAccountError::DuplicateSigner);
            }
            seen_signers.push(signer_key);
            seen_native_keys.push(signer_key);

            let signer_permissions = member.permissions();
            aggregated_permissions.mask |= signer_permissions.mask;

            if signer_permissions.has(Permission::Vote) {
                vote_permission_count += 1;
            }
        } else if let Some(external_signer) = consensus_account.find_signer_by_session_key(signer_key, current_timestamp) {
            // The native signer is a session key for an external signer.
            // Grant the external signer's permissions to this session key.
            let external_key_id = external_signer.key();

            // Check for duplicate (we track by external signer's key_id, not session key)
            if seen_signers.contains(&external_key_id) {
                return err!(SmartAccountError::DuplicateSigner);
            }
            seen_signers.push(external_key_id);

            let signer_permissions = external_signer.permissions();
            aggregated_permissions.mask |= signer_permissions.mask;

            if signer_permissions.has(Permission::Vote) {
                vote_permission_count += 1;
            }
        } else {
            return err!(SmartAccountError::NotASigner);
        }
    }

    // Validate external signers via precompile introspection
    if !args.external_signer_key_ids.is_empty() {
        // WebAuthn signers require client data params
        if args.client_data_params.is_none() {
            let needs_client_data = args
                .external_signer_key_ids
                .iter()
                .any(|key_id| {
                    consensus_account
                        .is_signer_v2(*key_id)
                        .map(|s| matches!(s.signer_type(), SignerType::P256Webauthn))
                        .unwrap_or(false)
                });
            if needs_client_data {
                return err!(SmartAccountError::MissingClientDataParams);
            }
        }

        let instructions_sysvar = instructions_sysvar
            .ok_or(SmartAccountError::MissingPrecompileInstruction)?;

        // Collect external signers from the consensus account
        let external_signers: Vec<SmartAccountSigner> = args
            .external_signer_key_ids
            .iter()
            .filter_map(|key_id| {
                // Find the signer in consensus account and check it's external
                consensus_account.is_signer_v2(*key_id).and_then(|s| {
                    if !matches!(s.signer_type(), SignerType::Native) {
                        Some(s)
                    } else {
                        None
                    }
                })
            })
            .collect();

        // Verify all claimed external signers were found
        require!(
            external_signers.len() == args.external_signer_key_ids.len(),
            SmartAccountError::NotASigner
        );

        // Create the expected message for sync consensus
        let expected_message = create_sync_consensus_message(
            &consensus_account_key,
            consensus_account.transaction_index(),
        );

        // Verify external signatures
        let verification = verify_external_signatures(
            instructions_sysvar,
            &external_signers,
            &expected_message,
            Some(&args.external_signer_key_ids),
            args.client_data_params.as_ref(),
        )?;

        // Collect counter updates for caller to persist
        counter_updates = verification.counter_updates;

        // Add permissions from external signers
        for signer in external_signers.iter() {
            // Check for duplicate
            let key = signer.key();
            if seen_signers.contains(&key) {
                return err!(SmartAccountError::DuplicateSigner);
            }
            // Prevent overlap between external signers and native signers (same key_id)
            if seen_native_keys.contains(&key) {
                return err!(SmartAccountError::DuplicateSigner);
            }
            seen_signers.push(key);

            let signer_permissions = signer.permissions();
            aggregated_permissions.mask |= signer_permissions.mask;

            if signer_permissions.has(Permission::Vote) {
                vote_permission_count += 1;
            }
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

    // Return the number of accounts consumed: instructions_sysvar (if used) + native signers
    let accounts_consumed = if instructions_sysvar.is_some() {
        1 + native_signer_count
    } else {
        native_signer_count
    };

    Ok(SyncConsensusV2Result {
        accounts_consumed,
        counter_updates,
    })
}

/// Collect signer pubkeys for event logging (native + external).
///
/// Layout of remaining_accounts must match sync consensus validation:
/// - First: instructions sysvar (if external signers used)
/// - Next `num_native_signers`: native signer accounts
pub fn collect_v2_signer_pubkeys(
    num_native_signers: u8,
    external_signer_key_ids: &[Pubkey],
    remaining_accounts: &[AccountInfo],
) -> Vec<Pubkey> {
    let has_instructions_sysvar = !external_signer_key_ids.is_empty();
    let native_start = if has_instructions_sysvar { 1 } else { 0 };
    let native_end = native_start + num_native_signers as usize;

    let mut signers: Vec<Pubkey> = remaining_accounts
        .get(native_start..native_end)
        .unwrap_or(&[])
        .iter()
        .map(|acc| *acc.key)
        .collect();
    signers.extend(external_signer_key_ids.iter().cloned());
    signers
}

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

    let mut seen_signers = Vec::with_capacity(signer_count);

    // Check signatures and membership only (V1 sync behavior)
    for signer in signers.iter() {
        if let Some(_member) = consensus_account.is_signer_v2(signer.key()) {
            if !signer.is_signer {
                return err!(SmartAccountError::MissingSignature);
            }
            if seen_signers.contains(&signer.key()) {
                return err!(SmartAccountError::DuplicateSigner);
            }
            seen_signers.push(signer.key());
        } else {
            return err!(SmartAccountError::NotASigner);
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        Ed25519ExternalData, P256WebauthnData, SessionKeyData, SmartAccountSigner,
        SmartAccountSignerWrapper, Settings,
    };
    use crate::{SEED_POLICY, SEED_PREFIX, SEED_SETTINGS};
    use solana_program::clock::Epoch;

    struct TestAccount {
        key: Pubkey,
        lamports: u64,
        data: Vec<u8>,
        owner: Pubkey,
        is_signer: bool,
        is_writable: bool,
    }

    impl TestAccount {
        fn new(key: Pubkey, is_signer: bool) -> Self {
            Self {
                key,
                lamports: 0,
                data: vec![],
                owner: Pubkey::new_unique(),
                is_signer,
                is_writable: false,
            }
        }

        fn to_account_info(&mut self) -> AccountInfo<'_> {
            AccountInfo::new(
                &self.key,
                self.is_signer,
                self.is_writable,
                &mut self.lamports,
                &mut self.data,
                &self.owner,
                false,
                Epoch::default(),
            )
        }
    }

    fn build_settings_with_signers(signers: SmartAccountSignerWrapper, threshold: u16) -> Settings {
        Settings {
            seed: 1,
            settings_authority: Pubkey::default(),
            threshold,
            time_lock: 0,
            transaction_index: 0,
            stale_transaction_index: 0,
            archival_authority: Some(Pubkey::default()),
            archivable_after: 0,
            bump: 255,
            signers,
            account_utilization: 0,
            policy_seed: Some(0),
            _reserved2: 0,
        }
    }

    #[test]
    fn sync_consensus_rejects_duplicate_session_keys() {
        let session_key = Pubkey::new_unique();
        let external_key_id = Pubkey::new_unique();

        let external_signer = SmartAccountSigner::Ed25519External {
            key_id: external_key_id,
            permissions: Permissions::all(),
            data: Ed25519ExternalData {
                external_pubkey: [7u8; 32],
                session_key_data: SessionKeyData {
                    key: session_key,
                    expiration: 200,
                },
            },
        };

        let settings = build_settings_with_signers(
            SmartAccountSignerWrapper::from_v2_signers(vec![external_signer]),
            1,
        );
        let consensus_account = ConsensusAccount::Settings(settings);

        let args = SyncConsensusV2Args {
            num_native_signers: 2,
            external_signer_key_ids: vec![],
            client_data_params: None,
        };

        let mut accounts = vec![
            TestAccount::new(session_key, true),
            TestAccount::new(session_key, true),
        ];
        let account_infos: Vec<AccountInfo> =
            accounts.iter_mut().map(|a| a.to_account_info()).collect();

        let result = validate_synchronous_consensus_v2_with_timestamp(
            &consensus_account,
            &args,
            consensus_account_key(&consensus_account),
            &account_infos,
            100,
        );

        assert!(matches!(result, Err(e) if e == error!(SmartAccountError::DuplicateSessionKey)));
    }

    #[test]
    fn sync_consensus_requires_client_data_for_webauthn() {
        let key_id = Pubkey::new_unique();
        let webauthn_signer = SmartAccountSigner::P256Webauthn {
            key_id,
            permissions: Permissions::all(),
            data: P256WebauthnData {
                compressed_pubkey: [3u8; 33],
                rp_id_len: 7,
                rp_id: {
                    let mut rp_id = [0u8; 32];
                    rp_id[..7].copy_from_slice(b"example");
                    rp_id
                },
                rp_id_hash: [4u8; 32],
                counter: 0,
                session_key_data: SessionKeyData::default(),
            },
        };

        let settings = build_settings_with_signers(
            SmartAccountSignerWrapper::from_v2_signers(vec![webauthn_signer]),
            1,
        );
        let consensus_account = ConsensusAccount::Settings(settings);

        let args = SyncConsensusV2Args {
            num_native_signers: 0,
            external_signer_key_ids: vec![key_id],
            client_data_params: None,
        };

        let result = validate_synchronous_consensus_v2_with_timestamp(
            &consensus_account,
            &args,
            consensus_account_key(&consensus_account),
            &[],
            100,
        );

        assert!(matches!(result, Err(e) if e == error!(SmartAccountError::MissingClientDataParams)));
    }

    #[test]
    fn sync_consensus_rejects_expired_session_key() {
        let session_key = Pubkey::new_unique();
        let external_key_id = Pubkey::new_unique();

        let external_signer = SmartAccountSigner::Ed25519External {
            key_id: external_key_id,
            permissions: Permissions::all(),
            data: Ed25519ExternalData {
                external_pubkey: [9u8; 32],
                session_key_data: SessionKeyData {
                    key: session_key,
                    expiration: 50,
                },
            },
        };

        let settings = build_settings_with_signers(
            SmartAccountSignerWrapper::from_v2_signers(vec![external_signer]),
            1,
        );
        let consensus_account = ConsensusAccount::Settings(settings);

        let args = SyncConsensusV2Args {
            num_native_signers: 1,
            external_signer_key_ids: vec![],
            client_data_params: None,
        };

        let mut accounts = vec![TestAccount::new(session_key, true)];
        let account_infos: Vec<AccountInfo> =
            accounts.iter_mut().map(|a| a.to_account_info()).collect();

        let result = validate_synchronous_consensus_v2_with_timestamp(
            &consensus_account,
            &args,
            consensus_account_key(&consensus_account),
            &account_infos,
            100,
        );

        assert!(matches!(result, Err(e) if e == error!(SmartAccountError::NotASigner)));
    }

    #[test]
    fn sync_consensus_requires_full_aggregate_permissions() {
        let signer_key = Pubkey::new_unique();

        let signer = SmartAccountSigner::Native {
            key: signer_key,
            permissions: Permissions { mask: Permission::Vote as u8 },
        };

        let settings = build_settings_with_signers(
            SmartAccountSignerWrapper::from_v2_signers(vec![signer]),
            1,
        );
        let consensus_account = ConsensusAccount::Settings(settings);

        let args = SyncConsensusV2Args {
            num_native_signers: 1,
            external_signer_key_ids: vec![],
            client_data_params: None,
        };

        let mut accounts = vec![TestAccount::new(signer_key, true)];
        let account_infos: Vec<AccountInfo> =
            accounts.iter_mut().map(|a| a.to_account_info()).collect();

        let result = validate_synchronous_consensus_v2_with_timestamp(
            &consensus_account,
            &args,
            consensus_account_key(&consensus_account),
            &account_infos,
            100,
        );

        assert!(matches!(result, Err(e) if e == error!(SmartAccountError::InsufficientAggregatePermissions)));
    }

    #[test]
    fn sync_consensus_requires_instructions_sysvar_for_external_signers() {
        let external_key_id = Pubkey::new_unique();

        let external_signer = SmartAccountSigner::Ed25519External {
            key_id: external_key_id,
            permissions: Permissions::all(),
            data: Ed25519ExternalData {
                external_pubkey: [11u8; 32],
                session_key_data: SessionKeyData::default(),
            },
        };

        let settings = build_settings_with_signers(
            SmartAccountSignerWrapper::from_v2_signers(vec![external_signer]),
            1,
        );
        let consensus_account = ConsensusAccount::Settings(settings);

        let args = SyncConsensusV2Args {
            num_native_signers: 0,
            external_signer_key_ids: vec![external_key_id],
            client_data_params: None,
        };

        let result = validate_synchronous_consensus_v2_with_timestamp(
            &consensus_account,
            &args,
            consensus_account_key(&consensus_account),
            &[],
            100,
        );

        assert!(matches!(result, Err(e) if e == error!(SmartAccountError::MissingPrecompileInstruction)));
    }

    #[test]
    fn sync_consensus_rejects_insufficient_vote_permissions_for_threshold() {
        let vote_key = Pubkey::new_unique();
        let execute_key = Pubkey::new_unique();

        let voter = SmartAccountSigner::Native {
            key: vote_key,
            permissions: Permissions::all(),
        };
        let executor = SmartAccountSigner::Native {
            key: execute_key,
            permissions: Permissions { mask: (Permission::Initiate as u8) | (Permission::Execute as u8) },
        };

        let settings = build_settings_with_signers(
            SmartAccountSignerWrapper::from_v2_signers(vec![voter, executor]),
            2,
        );
        let consensus_account = ConsensusAccount::Settings(settings);

        let args = SyncConsensusV2Args {
            num_native_signers: 2,
            external_signer_key_ids: vec![],
            client_data_params: None,
        };

        let mut accounts = vec![
            TestAccount::new(vote_key, true),
            TestAccount::new(execute_key, true),
        ];
        let account_infos: Vec<AccountInfo> =
            accounts.iter_mut().map(|a| a.to_account_info()).collect();

        let result = validate_synchronous_consensus_v2_with_timestamp(
            &consensus_account,
            &args,
            consensus_account_key(&consensus_account),
            &account_infos,
            100,
        );

        assert!(matches!(result, Err(e) if e == error!(SmartAccountError::InsufficientVotePermissions)));
    }

    fn consensus_account_key(consensus_account: &ConsensusAccount) -> Pubkey {
        match consensus_account {
            ConsensusAccount::Settings(settings) => {
                Pubkey::find_program_address(
                    &[SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
                    &crate::ID,
                )
                .0
            }
            ConsensusAccount::Policy(policy) => {
                Pubkey::find_program_address(
                    &[SEED_PREFIX, SEED_POLICY, policy.settings.as_ref(), &policy.seed.to_le_bytes()],
                    &crate::ID,
                )
                .0
            }
        }
    }
}
