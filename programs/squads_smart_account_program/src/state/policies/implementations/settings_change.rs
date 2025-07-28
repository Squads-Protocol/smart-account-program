use anchor_lang::prelude::*;

use crate::{
    errors::SmartAccountError, state::Settings, Permissions, PolicyExecutionContext,
    PolicyPayloadConversionTrait, PolicySizeTrait, PolicyTrait, SettingsAction, SmartAccountSigner,
};



/// == SettingsChangePolicy ==
/// This policy allows for the modification of the settings of a smart account.
///
/// The policy is defined by a set of allowed settings changes.
///===============================================

// =============================================================================
// CORE POLICY STRUCTURES
// =============================================================================
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct SettingsChangePolicy {
    pub actions: Vec<AllowedSettingsChange>,
}
/// Defines which settings changes are allowed by the policy
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug, InitSpace)]
pub enum AllowedSettingsChange {
    AddSigner {
        /// Some() - add a specific signer, None - add any signer
        new_signer: Option<Pubkey>,
        /// Some() - only allow certain permissions, None - allow all permissions
        new_signer_permissions: Option<Permissions>,
    },
    RemoveSigner {
        /// Some() - remove a specific signer, None - remove any signer
        old_signer: Option<Pubkey>,
    },
    ChangeThreshold,
    ChangeTimeLock {
        /// Some() - change timelock to a specific value, None - change timelock to any value
        new_time_lock: Option<u32>,
    },
}


// =============================================================================
// CREATION PAYLOAD TYPES
// =============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct SettingsChangePolicyCreationPayload {
    pub actions: Vec<AllowedSettingsChange>,
}

// =============================================================================
// EXECUTION PAYLOAD TYPES
// =============================================================================

/// Limited subset of settings change actions for execution
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum LimitedSettingsAction {
    AddSigner { new_signer: SmartAccountSigner },
    RemoveSigner { old_signer: Pubkey },
    ChangeThreshold { new_threshold: u16 },
    SetTimeLock { new_time_lock: u32 },
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct SettingsChangePayload {
    pub action_index: Vec<u8>,
    pub actions: Vec<LimitedSettingsAction>,
}

pub struct SettingsChangeExecutionArgs {
    pub settings_key: Pubkey,
}

pub struct ValidatedAccounts<'info> {
    pub settings: Account<'info, Settings>,
    /// Optional just to comply with later use of Settings::modify_with_action
    pub rent_payer: Option<Signer<'info>>,
    /// Optional just to comply with later use of Settings::modify_with_action
    pub system_program: Option<Program<'info, System>>,
}

// =============================================================================
// CONVERSION IMPLEMENTATIONS
// =============================================================================

impl From<LimitedSettingsAction> for SettingsAction {
    fn from(action: LimitedSettingsAction) -> Self {
        match action {
            LimitedSettingsAction::AddSigner { new_signer } => {
                SettingsAction::AddSigner { new_signer }
            }
            LimitedSettingsAction::RemoveSigner { old_signer } => {
                SettingsAction::RemoveSigner { old_signer }
            }
            LimitedSettingsAction::ChangeThreshold { new_threshold } => {
                SettingsAction::ChangeThreshold { new_threshold }
            }
            LimitedSettingsAction::SetTimeLock { new_time_lock } => {
                SettingsAction::SetTimeLock { new_time_lock }
            }
        }
    }
}

impl PolicyPayloadConversionTrait for SettingsChangePolicyCreationPayload {
    type PolicyState = SettingsChangePolicy;

    fn to_policy_state(self) -> Result<SettingsChangePolicy> {
        let mut sorted_actions = self.actions.clone();
        // Sort the actions to ensure the invariant function can apply
        sorted_actions.sort_by_key(|action| match action {
            AllowedSettingsChange::AddSigner { new_signer, .. } => (0, new_signer.clone()),
            AllowedSettingsChange::RemoveSigner { old_signer } => (1, old_signer.clone()),
            AllowedSettingsChange::ChangeThreshold => (2, None),
            AllowedSettingsChange::ChangeTimeLock { .. } => (3, None),
        });
        Ok(SettingsChangePolicy {
            actions: sorted_actions,
        })
    }
}

impl PolicySizeTrait for SettingsChangePolicyCreationPayload {
    fn creation_payload_size(&self) -> usize {
        4 + self.actions.len() * AllowedSettingsChange::INIT_SPACE // actions vec
    }

    fn policy_state_size(&self) -> usize {
        // Same as creation payload size
        self.creation_payload_size()
    }
}

// =============================================================================
// POLICY TRAIT IMPLEMENTATION
// =============================================================================
impl PolicyTrait for SettingsChangePolicy {
    type PolicyState = Self;
    type CreationPayload = SettingsChangePolicyCreationPayload;
    type UsagePayload = SettingsChangePayload;
    type ExecutionArgs = SettingsChangeExecutionArgs;

    /// Validate policy invariants - no duplicate actions
    fn invariant(&self) -> Result<()> {
        // Check for adjacent duplicates (assumes sorted actions by enum and pubkey)
        // Rules:
        // - AddSigner and RemoveSigner can only be present once with any given pubkey
        // - ChangeThreshold and ChangeTimeLock can only each be present once
        let has_duplicate = self.actions.windows(2).any(|win| match (&win[0], &win[1]) {
            (
                AllowedSettingsChange::AddSigner {
                    new_signer: signer1,
                    ..
                },
                AllowedSettingsChange::AddSigner {
                    new_signer: signer2,
                    ..
                },
            ) => signer1 == signer2,
            (
                AllowedSettingsChange::RemoveSigner {
                    old_signer: signer1,
                },
                AllowedSettingsChange::RemoveSigner {
                    old_signer: signer2,
                },
            ) => signer1 == signer2,
            (AllowedSettingsChange::ChangeThreshold, AllowedSettingsChange::ChangeThreshold) => {
                true
            }
            (
                AllowedSettingsChange::ChangeTimeLock { .. },
                AllowedSettingsChange::ChangeTimeLock { .. },
            ) => true,
            _ => false,
        });

        if has_duplicate {
            return Err(SmartAccountError::SettingsChangePolicyInvariantDuplicateActions.into());
        }
        Ok(())
    }

    /// Validate that the payload actions match allowed policy actions
    fn validate_payload(
        &self,
        // No difference between synchronous and asynchronous execution
        _context: PolicyExecutionContext,
        payload: &Self::UsagePayload,
    ) -> Result<()> {
        // Actions need to be non-zero
        require!(
            !payload.actions.is_empty(),
            SmartAccountError::SettingsChangePolicyActionsMustBeNonZero
        );
        // Action indices must match actions length
        require!(
            payload.action_index.len() == payload.actions.len(),
            SmartAccountError::SettingsChangePolicyInvariantActionIndicesActionsLengthMismatch
        );

        // This is safe because we checked that the action indices match the actions length
        for (action_index, action) in payload.action_index.iter().zip(payload.actions.iter()) {
            // Get the corresponding action from the policy state
            let allowed_action = if let Some(action) = self.actions.get(*action_index as usize) {
                action
            } else {
                return Err(
                    SmartAccountError::SettingsChangePolicyInvariantActionIndexOutOfBounds.into(),
                );
            };
            match (allowed_action, action) {
                (
                    AllowedSettingsChange::AddSigner {
                        new_signer: allowed_signer,
                        new_signer_permissions: allowed_permissions,
                    },
                    LimitedSettingsAction::AddSigner { new_signer },
                ) => {
                    if let Some(allowed_signer) = allowed_signer {
                        // If None, any signer can be added
                        require!(
                            &new_signer.key == allowed_signer,
                            SmartAccountError::SettingsChangeAddSignerViolation
                        );
                    }
                    // If None, any permissions can used
                    if let Some(allowed_permissions) = allowed_permissions {
                        require!(
                            &new_signer.permissions == allowed_permissions,
                            SmartAccountError::SettingsChangeAddSignerPermissionsViolation
                        );
                    }
                }
                (
                    AllowedSettingsChange::RemoveSigner {
                        old_signer: allowed_removal_signer,
                    },
                    LimitedSettingsAction::RemoveSigner { old_signer },
                ) => {
                    // If None, any signer can be removed
                    if let Some(allowed_removal_signer) = allowed_removal_signer {
                        require!(
                            old_signer == allowed_removal_signer,
                            SmartAccountError::SettingsChangeRemoveSignerViolation
                        );
                    }
                }
                (
                    AllowedSettingsChange::ChangeThreshold,
                    LimitedSettingsAction::ChangeThreshold { new_threshold: _ },
                ) => {
                    continue;
                }
                (
                    AllowedSettingsChange::ChangeTimeLock {
                        new_time_lock: allowed_time_lock,
                    },
                    LimitedSettingsAction::SetTimeLock { new_time_lock },
                ) => {
                    // If None, any time lock can be used
                    if let Some(allowed_time_lock) = allowed_time_lock {
                        require!(
                            new_time_lock == allowed_time_lock,
                            SmartAccountError::SettingsChangeChangeTimelockViolation
                        );
                    }
                }
                _ => {
                    return Err(SmartAccountError::SettingsChangeActionMismatch.into());
                }
            }
        }
        Ok(())
    }

    /// Execute the settings change actions
    fn execute_payload<'info>(
        &mut self,
        args: Self::ExecutionArgs,
        payload: &Self::UsagePayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        // Validate and grab the settings account
        let mut validated_accounts = self.validate_accounts(args.settings_key, accounts)?;
        for action in payload.actions.iter() {
            let settings_action = SettingsAction::from(action.clone());
            validated_accounts.settings.modify_with_action(
                &args.settings_key,
                &settings_action,
                &Rent::get()?,
                &validated_accounts.rent_payer,
                &validated_accounts.system_program,
                // Only policies and spending limits use remaining accounts, and
                // those actions are excluded from LimitedSettingsAction
                &[],
                &crate::ID,
            )?;
        }
        Ok(())
    }
}

// =============================================================================
// ACCOUNT VALIDATION
// =============================================================================

impl SettingsChangePolicy {
    /// Validate the accounts needed for settings change execution
    pub fn validate_accounts<'info>(
        &self,
        settings_key: Pubkey,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<ValidatedAccounts<'info>> {
        let (settings_account_info, rent_payer_info, system_program_info) = if let [settings_account_info, rent_payer_info, system_program_info, _remaining @ ..] =
            accounts
        {
            (settings_account_info, rent_payer_info, system_program_info)
        } else {
            return err!(SmartAccountError::InvalidNumberOfAccounts);
        };

        // Settings account validation
        require!(
            settings_account_info.key() == settings_key,
            SmartAccountError::SettingsChangeInvalidSettingsKey
        );
        require!(
            settings_account_info.is_writable,
            SmartAccountError::SettingsChangeInvalidSettingsAccount
        );
        let settings: Account<'info, Settings> = Account::try_from(settings_account_info)?;

        // Rent payer validation
        let rent_payer = Signer::try_from(rent_payer_info)
            .map_err(|_| SmartAccountError::SettingsChangeInvalidRentPayer)?;
        require!(
            rent_payer.is_writable,
            SmartAccountError::SettingsChangeInvalidRentPayer
        );

        // System program validation
        let system_program: Program<'info, System> = Program::try_from(system_program_info)
            .map_err(|_| SmartAccountError::SettingsChangeInvalidSystemProgram)?;

        Ok(ValidatedAccounts {
            settings,
            rent_payer: Some(rent_payer),
            system_program: Some(system_program),
        })
    }
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use crate::Permission;

    use super::*;

    #[test]
    fn test_invariant_valid_configuration() {
        let payload = SettingsChangePolicyCreationPayload {
            actions: vec![
                AllowedSettingsChange::AddSigner {
                    new_signer: Some(Pubkey::new_unique()),
                    new_signer_permissions: None,
                },
                AllowedSettingsChange::RemoveSigner {
                    old_signer: Some(Pubkey::new_unique()),
                },
                AllowedSettingsChange::ChangeThreshold,
                AllowedSettingsChange::ChangeTimeLock {
                    new_time_lock: Some(1800),
                },
            ],
        };

        let policy = payload.to_policy_state().unwrap();
        assert!(policy.invariant().is_ok());
    }
    #[test]
    fn test_invariant_duplicate_add_signer_same_pubkey() {
        let duplicate_signer = Pubkey::new_unique();
        let payload = SettingsChangePolicyCreationPayload {
            actions: vec![
                AllowedSettingsChange::AddSigner {
                    new_signer: Some(duplicate_signer),
                    new_signer_permissions: None,
                },
                AllowedSettingsChange::RemoveSigner {
                    old_signer: Some(duplicate_signer),
                },
                AllowedSettingsChange::AddSigner {
                    new_signer: Some(duplicate_signer),
                    new_signer_permissions: Some(Permissions::from_vec(&[Permission::Initiate])),
                },
            ],
        };

        let policy = payload.to_policy_state().unwrap();
        assert!(policy.invariant().is_err());
    }

    #[test]
    fn test_invariant_duplicate_remove_signer_same_pubkey_out_of_order() {
        let duplicate_signer = Pubkey::new_unique();
        let payload = SettingsChangePolicyCreationPayload {
            actions: vec![
                AllowedSettingsChange::RemoveSigner {
                    old_signer: Some(duplicate_signer),
                },
                AllowedSettingsChange::AddSigner {
                    new_signer: Some(duplicate_signer),
                    new_signer_permissions: None,
                },
                AllowedSettingsChange::RemoveSigner {
                    old_signer: Some(duplicate_signer),
                },
            ],
        };

        let policy = payload.to_policy_state().unwrap();
        assert!(policy.invariant().is_err());
    }

    #[test]
    fn test_invariant_duplicate_none_values_invalid() {
        let payload = SettingsChangePolicyCreationPayload {
            actions: vec![
                AllowedSettingsChange::AddSigner {
                    new_signer: None,
                    new_signer_permissions: Some(Permissions::from_vec(&[Permission::Initiate])),
                },
                AllowedSettingsChange::AddSigner {
                    new_signer: None,
                    new_signer_permissions: Some(Permissions::from_vec(&[Permission::Execute])),
                },
                AllowedSettingsChange::RemoveSigner { old_signer: None },
                AllowedSettingsChange::RemoveSigner { old_signer: None },
            ],
        };

        let policy = payload.to_policy_state().unwrap();
        assert!(policy.invariant().is_err());
    }

    #[test]
    fn test_invariant_duplicate_change_time_lock() {
        let payload = SettingsChangePolicyCreationPayload {
            actions: vec![
                AllowedSettingsChange::ChangeTimeLock {
                    new_time_lock: Some(1800),
                },
                AllowedSettingsChange::ChangeTimeLock {
                    new_time_lock: Some(3600),
                },
            ],
        };

        let policy = payload.to_policy_state().unwrap();
        assert!(policy.invariant().is_err());
    }

    #[test]
    fn test_creation_payload_size_calculation() {
        let payload = SettingsChangePolicyCreationPayload {
            actions: vec![
                AllowedSettingsChange::AddSigner {
                    new_signer: Some(Pubkey::new_unique()),
                    new_signer_permissions: Some(Permissions::all()),
                },
                AllowedSettingsChange::RemoveSigner {
                    old_signer: Some(Pubkey::new_unique()),
                },
                AllowedSettingsChange::ChangeThreshold,
                AllowedSettingsChange::ChangeTimeLock {
                    new_time_lock: Some(3600),
                },
            ],
        };

        let calculated_size = payload.creation_payload_size();
        let actual_serialized = payload.try_to_vec().unwrap();
        let actual_size = actual_serialized.len();

        assert!(calculated_size >= actual_size);
    }

    #[test]
    fn test_policy_state_size_calculation() {
        let payload = SettingsChangePolicyCreationPayload {
            actions: vec![
                AllowedSettingsChange::AddSigner {
                    new_signer: Some(Pubkey::new_unique()),
                    new_signer_permissions: Some(Permissions::all()),
                },
                AllowedSettingsChange::RemoveSigner {
                    old_signer: Some(Pubkey::new_unique()),
                },
                AllowedSettingsChange::ChangeThreshold,
                AllowedSettingsChange::ChangeTimeLock {
                    new_time_lock: Some(3600),
                },
            ],
        };

        let policy = payload.clone().to_policy_state().unwrap();
        let calculated_size = payload.policy_state_size();
        let actual_serialized = policy.try_to_vec().unwrap();
        let actual_size = actual_serialized.len();

        // Since InitSpace overestimates size, we only check that the calculated
        // size is greater than or equal to the actual size to make sure
        // serialization succeeds
        assert!(calculated_size >= actual_size);
    }
}
