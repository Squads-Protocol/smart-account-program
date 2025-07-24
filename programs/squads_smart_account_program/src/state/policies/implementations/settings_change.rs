use anchor_lang::prelude::*;

use crate::{
    errors::SmartAccountError, policy_core, state::Settings, Permission, Permissions,
    PolicyPayloadConversionTrait, PolicySizeTrait, PolicyTrait, SettingsAction, SmartAccountSigner,
};

/// Supports a subset of settings changes that can be done via a policy.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug, InitSpace)]
pub enum AllowedSettingsChange {
    AddSigner {
        // Some() - add a specific signer
        // None - add any signer
        new_signer: Option<Pubkey>,
        // Some() - only allow certain permissions
        // None - allow all permissions
        new_signer_permissions: Option<Permissions>,
    },
    RemoveSigner {
        // Some() - remove a specific signer
        // None - remove any signer
        old_signer: Option<Pubkey>,
    },
    ChangeThreshold,
    ChangeTimeLock {
        // Some() - change timelock to a specific value
        // None - change timelock to any value
        new_time_lock: Option<u32>,
    },
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct SettingsChangePolicyCreationPayload {
    pub actions: Vec<AllowedSettingsChange>,
}

impl PolicyPayloadConversionTrait for SettingsChangePolicyCreationPayload {
    type PolicyState = SettingsChangePolicy;

    fn to_policy_state(self) -> SettingsChangePolicy {
        let mut sorted_actions = self.actions.clone();
        sorted_actions.sort_by_key(|action| match action {
            AllowedSettingsChange::AddSigner { new_signer, .. } => (0, new_signer.clone()),
            AllowedSettingsChange::RemoveSigner { old_signer } => (1, old_signer.clone()),
            AllowedSettingsChange::ChangeThreshold => (2, None),
            AllowedSettingsChange::ChangeTimeLock { .. } => (3, None),
        });
        SettingsChangePolicy {
            actions: sorted_actions,
        }
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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct SettingsChangePolicy {
    pub actions: Vec<AllowedSettingsChange>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
/// Payload used to update the settings.
pub enum SettingsChangeActions {
    AddSigner {
        new_signer: Pubkey,
        new_signer_permissions: Permissions,
    },
    RemoveSigner {
        old_signer: Pubkey,
    },
    ChangeThreshold {
        new_threshold: u16,
    },
    ChangeTimeLock {
        new_time_lock: u32,
    },
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct SettingsChangePayload {
    pub actions: Vec<SettingsChangeActions>,
}
impl PolicyTrait for SettingsChangePolicy {
    type PolicyState = Self;
    type CreationPayload = SettingsChangePolicyCreationPayload;
    type UsagePayload = SettingsChangePayload;
    type ExecutionArgs = ();

    fn invariant(&self) -> Result<()> {
        // AddSigner and RemoveSigner can only be present once with any given
        // pubkey.
        // ChangeThreshold and ChangeTimeLock can only each be present once.
        // Assumes sorted actions by enum and pubkey.

        // There must be no duplicate signers.
        // AddSigner and RemoveSigner can only be present once with any given
        // pubkey.
        // ChangeThreshold and ChangeTimeLock can only each be present once.
        // Assumes sorted actions by enum and pubkey.

        // Check for adjacent duplicates
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
            return Err(SmartAccountError::PlaceholderError.into());
        }
        Ok(())
    }

    fn validate_payload(&self, payload: &Self::UsagePayload) -> Result<()> {
        // for action in payload {
        //     match action {
        //         SettingsChangePayload::AddSigner { new_signer, new_signer_permissions } => {
        //             if self
        //         }
        //         SettingsChangePayload::RemoveSigner { old_signer } => {
        //             if let Some(old_signer) = old_signer {
        //                 require_eq!(old_signer, self.settings_change_type.settings_authority);
        //             }
        //         }
        //         SettingsChangePayload::ChangeThreshold => {}
        //         SettingsChangePayload::ChangeTimeLock { new_time_lock } => {}
        //     }
        // }
        Ok(())
    }

    fn execute_payload<'info>(
        &mut self,
        args: Self::ExecutionArgs,
        payload: &Self::UsagePayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        Ok(())
    }


}

#[cfg(test)]
mod tests {
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

        let policy = payload.to_policy_state();
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

        let policy = payload.to_policy_state();
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

        let policy = payload.to_policy_state();
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

        let policy = payload.to_policy_state();
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

        let policy = payload.to_policy_state();
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

        let policy = payload.clone().to_policy_state();
        let calculated_size = payload.policy_state_size();
        let actual_serialized = policy.try_to_vec().unwrap();
        let actual_size = actual_serialized.len();

        // Since InitSpace overestimates size, we only check that the calculated
        // size is greater than or equal to the actual size to make sure
        // serialization succeeds
        assert!(calculated_size >= actual_size);
    }
}
