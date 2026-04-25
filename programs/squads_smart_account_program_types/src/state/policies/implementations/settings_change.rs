use solana_program::pubkey::Pubkey;

use crate::errors::SmartAccountError;
use crate::state::policies::policy_core::{
    PolicyExecutionContext, PolicyPayloadConversionTrait, PolicySizeTrait,
};
use crate::state::{Permissions, SettingsAction, SmartAccountSigner};

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SettingsChangePolicy {
    pub actions: Vec<AllowedSettingsChange>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AllowedSettingsChange {
    AddSigner {
        new_signer: Option<Pubkey>,
        new_signer_permissions: Option<Permissions>,
    },
    RemoveSigner {
        old_signer: Option<Pubkey>,
    },
    ChangeThreshold,
    ChangeTimeLock {
        new_time_lock: Option<u32>,
    },
}

impl AllowedSettingsChange {
    // Max variant:
    // AddSigner: 1 (Option discriminator) + 32 (Pubkey) + 1 (Option discriminator) + 1 (Permissions) = 35
    // RemoveSigner: 1 + 32 = 33
    // ChangeThreshold: 0
    // ChangeTimeLock: 1 + 4 = 5
    // So 1 (enum disc) + 35 = 36
    pub const INIT_SPACE: usize = 1 + 35;
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SettingsChangePolicyCreationPayload {
    pub actions: Vec<AllowedSettingsChange>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq)]
pub enum LimitedSettingsAction {
    AddSigner { new_signer: SmartAccountSigner },
    RemoveSigner { old_signer: Pubkey },
    ChangeThreshold { new_threshold: u16 },
    SetTimeLock { new_time_lock: u32 },
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq)]
pub struct SettingsChangePayload {
    pub action_index: Vec<u8>,
    pub actions: Vec<LimitedSettingsAction>,
}

pub struct SettingsChangeExecutionArgs {
    pub settings_key: Pubkey,
}

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

    fn to_policy_state(self) -> Result<SettingsChangePolicy, SmartAccountError> {
        let mut sorted_actions = self.actions.clone();
        sorted_actions.sort_by_key(|action| match action {
            AllowedSettingsChange::AddSigner { new_signer, .. } => (0, *new_signer),
            AllowedSettingsChange::RemoveSigner { old_signer } => (1, *old_signer),
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
        4 + self.actions.len() * AllowedSettingsChange::INIT_SPACE
    }

    fn policy_state_size(&self) -> usize {
        self.creation_payload_size()
    }
}

impl SettingsChangePolicy {
    pub fn invariant(&self) -> Result<(), SmartAccountError> {
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
            return Err(SmartAccountError::SettingsChangePolicyInvariantDuplicateActions);
        }
        Ok(())
    }

    pub fn validate_payload(
        &self,
        _context: PolicyExecutionContext,
        payload: &SettingsChangePayload,
    ) -> Result<(), SmartAccountError> {
        if payload.actions.is_empty() {
            return Err(SmartAccountError::SettingsChangePolicyActionsMustBeNonZero);
        }
        if payload.action_index.len() != payload.actions.len() {
            return Err(
                SmartAccountError::SettingsChangePolicyInvariantActionIndicesActionsLengthMismatch,
            );
        }

        for (action_index, action) in payload.action_index.iter().zip(payload.actions.iter()) {
            let allowed_action = self
                .actions
                .get(*action_index as usize)
                .ok_or(SmartAccountError::SettingsChangePolicyInvariantActionIndexOutOfBounds)?;
            match (allowed_action, action) {
                (
                    AllowedSettingsChange::AddSigner {
                        new_signer: allowed_signer,
                        new_signer_permissions: allowed_permissions,
                    },
                    LimitedSettingsAction::AddSigner { new_signer },
                ) => {
                    if let Some(allowed_signer) = allowed_signer {
                        if &new_signer.key != allowed_signer {
                            return Err(SmartAccountError::SettingsChangeAddSignerViolation);
                        }
                    }
                    if let Some(allowed_permissions) = allowed_permissions {
                        if &new_signer.permissions != allowed_permissions {
                            return Err(
                                SmartAccountError::SettingsChangeAddSignerPermissionsViolation,
                            );
                        }
                    }
                }
                (
                    AllowedSettingsChange::RemoveSigner {
                        old_signer: allowed_removal_signer,
                    },
                    LimitedSettingsAction::RemoveSigner { old_signer },
                ) => {
                    if let Some(allowed_removal_signer) = allowed_removal_signer {
                        if old_signer != allowed_removal_signer {
                            return Err(SmartAccountError::SettingsChangeRemoveSignerViolation);
                        }
                    }
                }
                (
                    AllowedSettingsChange::ChangeThreshold,
                    LimitedSettingsAction::ChangeThreshold { new_threshold: _ },
                ) => {}
                (
                    AllowedSettingsChange::ChangeTimeLock {
                        new_time_lock: allowed_time_lock,
                    },
                    LimitedSettingsAction::SetTimeLock { new_time_lock },
                ) => {
                    if let Some(allowed_time_lock) = allowed_time_lock {
                        if new_time_lock != allowed_time_lock {
                            return Err(SmartAccountError::SettingsChangeChangeTimelockViolation);
                        }
                    }
                }
                _ => {
                    return Err(SmartAccountError::SettingsChangeActionMismatch);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Permission, Permissions};

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
}
