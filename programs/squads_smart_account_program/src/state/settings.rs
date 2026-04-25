//! `Settings` — re-exported from the types crate. The extension trait
//! `SettingsExt` holds methods that need `Clock::get`, `AccountInfo`, or
//! program CPI helpers. The impl of `Consensus` for the foreign `Settings` is
//! also kept here (local trait, foreign type — OK by the orphan rule).

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use solana_program::hash::hash;

pub use squads_smart_account_program_types::{
    Permission, Permissions, Settings, SmartAccountSigner, MAX_TIME_LOCK,
};

use crate::error_conv::ToAnchorResult;
use crate::AddSpendingLimitEvent;
use crate::LogAuthorityInfo;
use crate::PolicyEvent;
use crate::PolicyEventType;
use crate::RemoveSpendingLimitEvent;
use crate::SmartAccountEvent;
use crate::SmartAccountEventExt;
use crate::{
    errors::*,
    id,
    interface::consensus_trait::{Consensus, ConsensusAccountType},
    state::*,
    utils::*,
    SettingsAction,
};

/// Program-side extension methods for `Settings` — use Clock, AccountInfo,
/// and other anchor-side helpers that can't live in the pure types crate.
pub trait SettingsExt {
    fn generate_core_state_hash(&self) -> Result<[u8; 32]>;
    fn find_and_initialize_settings_account<'info>(
        &self,
        settings_account_key: Pubkey,
        rent_payer: &AccountInfo<'info>,
        remaining_accounts: &'info [AccountInfo<'info>],
        system_program: &Program<'info, System>,
    ) -> Result<&'info AccountInfo<'info>>;
    fn realloc_if_needed<'a>(
        settings: AccountInfo<'a>,
        signers_length: usize,
        rent_payer: Option<AccountInfo<'a>>,
        system_program: Option<AccountInfo<'a>>,
    ) -> Result<bool>;
    fn modify_with_action<'info>(
        &mut self,
        self_key: &Pubkey,
        action: &SettingsAction,
        rent: &Rent,
        rent_payer: &Option<Signer<'info>>,
        system_program: &Option<Program<'info, System>>,
        remaining_accounts: &'info [AccountInfo<'info>],
        program_id: &Pubkey,
        log_authority_info: Option<&LogAuthorityInfo<'info>>,
    ) -> Result<()>;
}

impl SettingsExt for Settings {
    /// Hashes the core settings (signers, threshold, time_lock) for expiration checks.
    fn generate_core_state_hash(&self) -> Result<[u8; 32]> {
        let mut data_to_hash = Vec::new();

        // Signers
        for signer in &self.signers {
            data_to_hash.extend_from_slice(signer.key.as_ref());
            // Add signer permissions (1 byte)
            data_to_hash.push(signer.permissions.mask);
        }
        // Threshold
        data_to_hash.extend_from_slice(&self.threshold.to_le_bytes());

        // Timelock
        data_to_hash.extend_from_slice(&self.time_lock.to_le_bytes());
        let hash_result = hash(&data_to_hash);
        Ok(hash_result.to_bytes())
    }

    fn find_and_initialize_settings_account<'info>(
        &self,
        settings_account_key: Pubkey,
        rent_payer: &AccountInfo<'info>,
        remaining_accounts: &'info [AccountInfo<'info>],
        system_program: &Program<'info, System>,
    ) -> Result<&'info AccountInfo<'info>> {
        let settings_account_info = remaining_accounts
            .iter()
            .find(|acc| acc.key == &settings_account_key)
            .ok_or(SmartAccountError::MissingAccount)?;

        // Assert that the account is uninitialized and marked as writable
        require!(
            settings_account_info.owner == &system_program::ID,
            ErrorCode::AccountNotSystemOwned
        );
        require!(
            settings_account_info.data_is_empty(),
            SmartAccountError::AccountNotEmpty
        );
        require!(
            settings_account_info.is_writable,
            ErrorCode::AccountNotMutable
        );

        let rent = Rent::get()?;

        create_account(
            rent_payer,
            settings_account_info,
            system_program,
            &crate::ID,
            &rent,
            Settings::size(self.signers.len()),
            vec![
                SEED_PREFIX.to_vec(),
                SEED_SETTINGS.to_vec(),
                self.seed.to_le_bytes().to_vec(),
                vec![self.bump],
            ],
        )?;

        Ok(settings_account_info)
    }

    /// Check if the settings account space needs to be reallocated.
    fn realloc_if_needed<'a>(
        settings: AccountInfo<'a>,
        signers_length: usize,
        rent_payer: Option<AccountInfo<'a>>,
        system_program: Option<AccountInfo<'a>>,
    ) -> Result<bool> {
        // Sanity checks
        require_keys_eq!(
            *settings.owner,
            id(),
            SmartAccountError::IllegalAccountOwner
        );

        let current_account_size = settings.data.borrow().len();
        let account_size_to_fit_signers = Settings::size(signers_length);

        // Check if we need to reallocate space.
        if current_account_size >= account_size_to_fit_signers {
            return Ok(false);
        }

        let new_size = account_size_to_fit_signers;

        // Reallocate more space.
        realloc(&settings, new_size, rent_payer, system_program)?;
        Ok(true)
    }

    // Modify the settings with a given action.
    fn modify_with_action<'info>(
        &mut self,
        self_key: &Pubkey,
        action: &SettingsAction,
        rent: &Rent,
        rent_payer: &Option<Signer<'info>>,
        system_program: &Option<Program<'info, System>>,
        remaining_accounts: &'info [AccountInfo<'info>],
        program_id: &Pubkey,
        log_authority_info: Option<&LogAuthorityInfo<'info>>,
    ) -> Result<()> {
        match action {
            SettingsAction::AddSigner { new_signer } => {
                self.add_signer(new_signer.to_owned());
                self.invalidate_prior_transactions();
            }

            SettingsAction::RemoveSigner { old_signer } => {
                self.remove_signer(old_signer.to_owned()).to_anchor()?;
                self.invalidate_prior_transactions();
            }

            SettingsAction::ChangeThreshold { new_threshold } => {
                self.threshold = *new_threshold;
                self.invalidate_prior_transactions();
            }

            SettingsAction::SetTimeLock { new_time_lock } => {
                self.time_lock = *new_time_lock;
                self.invalidate_prior_transactions();
            }

            SettingsAction::AddSpendingLimit {
                seed,
                account_index,
                signers,
                mint,
                amount,
                period,
                destinations,
                expiration,
            } => {
                let (spending_limit_key, spending_limit_bump) = Pubkey::find_program_address(
                    &[
                        SEED_PREFIX,
                        self_key.as_ref(),
                        SEED_SPENDING_LIMIT,
                        seed.as_ref(),
                    ],
                    program_id,
                );

                let spending_limit_info = remaining_accounts
                    .iter()
                    .find(|acc| acc.key == &spending_limit_key)
                    .ok_or(SmartAccountError::MissingAccount)?;

                let rent_payer = rent_payer
                    .as_ref()
                    .ok_or(SmartAccountError::MissingAccount)?;
                let system_program = system_program
                    .as_ref()
                    .ok_or(SmartAccountError::MissingAccount)?;

                create_account(
                    &rent_payer.to_account_info(),
                    spending_limit_info,
                    &system_program.to_account_info(),
                    &id(),
                    rent,
                    SpendingLimit::size(signers.len(), destinations.len()),
                    vec![
                        SEED_PREFIX.to_vec(),
                        self_key.as_ref().to_vec(),
                        SEED_SPENDING_LIMIT.to_vec(),
                        seed.as_ref().to_vec(),
                        vec![spending_limit_bump],
                    ],
                )?;

                let mut signers = signers.to_vec();
                signers.sort();

                let spending_limit = SpendingLimit {
                    settings: self_key.to_owned(),
                    seed: seed.to_owned(),
                    account_index: *account_index,
                    signers,
                    amount: *amount,
                    mint: *mint,
                    period: *period,
                    remaining_amount: *amount,
                    last_reset: Clock::get()?.unix_timestamp,
                    bump: spending_limit_bump,
                    destinations: destinations.to_vec(),
                    expiration: *expiration,
                };

                spending_limit.invariant().to_anchor()?;
                spending_limit
                    .try_serialize(&mut &mut spending_limit_info.data.borrow_mut()[..])?;

                let event = AddSpendingLimitEvent {
                    settings_pubkey: self_key.to_owned(),
                    spending_limit_pubkey: spending_limit_key,
                    spending_limit: spending_limit.clone(),
                };
                if let Some(log_authority_info) = log_authority_info {
                    SmartAccountEvent::AddSpendingLimitEvent(event).log(log_authority_info)?;
                }
            }

            SettingsAction::RemoveSpendingLimit {
                spending_limit: spending_limit_key,
            } => {
                let spending_limit_info = remaining_accounts
                    .iter()
                    .find(|acc| acc.key == spending_limit_key)
                    .ok_or(SmartAccountError::MissingAccount)?;

                let rent_payer = rent_payer
                    .as_ref()
                    .ok_or(SmartAccountError::MissingAccount)?;

                let spending_limit = Account::<SpendingLimit>::try_from(spending_limit_info)?;

                require_keys_eq!(
                    spending_limit.settings,
                    self_key.to_owned(),
                    SmartAccountError::InvalidAccount
                );

                spending_limit.close(rent_payer.to_account_info())?;

                // Log the closing event
                let event = RemoveSpendingLimitEvent {
                    settings_pubkey: self_key.to_owned(),
                    spending_limit_pubkey: *spending_limit_key,
                };
                if let Some(log_authority_info) = log_authority_info {
                    SmartAccountEvent::RemoveSpendingLimitEvent(event).log(log_authority_info)?;
                }
            }

            SettingsAction::SetArchivalAuthority {
                new_archival_authority: _,
            } => {
                // Marked as NotImplemented until archival feature is implemented.
                return err!(SmartAccountError::NotImplemented);
            }

            SettingsAction::PolicyCreate {
                policy_creation_payload,
                signers,
                threshold,
                time_lock,
                start_timestamp,
                expiration_args,
                ..
            } => {
                // Increment the policy seed if it exists, otherwise set it to
                // 1 (First policy is being created)
                let next_policy_seed = if let Some(policy_seed) = self.policy_seed {
                    let next_policy_seed = policy_seed.checked_add(1).unwrap();

                    // Increment the policy seed
                    self.policy_seed = Some(next_policy_seed);
                    next_policy_seed
                } else {
                    self.policy_seed = Some(1);
                    1
                };
                // Policies get created at a deterministic address based on the
                // seed in the settings.

                let (policy_pubkey, policy_bump) = Pubkey::find_program_address(
                    &[
                        crate::SEED_PREFIX,
                        SEED_POLICY,
                        self_key.as_ref(),
                        &next_policy_seed.to_le_bytes(),
                    ],
                    program_id,
                );

                let policy_info = remaining_accounts
                    .iter()
                    .find(|acc| acc.key == &policy_pubkey)
                    .ok_or(SmartAccountError::MissingAccount)?;

                // Calculate policy data size based on the creation payload
                let policy_specific_data_size = policy_creation_payload.policy_state_size();

                let policy_size = Policy::size(signers.len(), policy_specific_data_size);

                let rent_payer = rent_payer
                    .as_ref()
                    .ok_or(SmartAccountError::MissingAccount)?;
                let system_program = system_program
                    .as_ref()
                    .ok_or(SmartAccountError::MissingAccount)?;

                // Create the policy account (following the pattern from create_spending_limit)
                create_account(
                    &rent_payer.to_account_info(),
                    policy_info,
                    &system_program.to_account_info(),
                    &id(),
                    rent,
                    policy_size,
                    vec![
                        crate::SEED_PREFIX.to_vec(),
                        SEED_POLICY.to_vec(),
                        self_key.as_ref().to_vec(),
                        next_policy_seed.to_le_bytes().to_vec(),
                        vec![policy_bump],
                    ],
                )?;

                // Convert creation payload to policy type
                // TODO: Get rid of this clone
                let policy_state = match policy_creation_payload.clone() {
                    PolicyCreationPayload::InternalFundTransfer(creation_payload) => {
                        PolicyState::InternalFundTransfer(
                            creation_payload.to_policy_state().to_anchor()?,
                        )
                    }
                    PolicyCreationPayload::ProgramInteraction(creation_payload) => {
                        PolicyState::ProgramInteraction(
                            crate::state::policies::program_interaction_creation_to_policy_state(
                                creation_payload,
                            )?,
                        )
                    }
                    PolicyCreationPayload::SpendingLimit(creation_payload) => {
                        PolicyState::SpendingLimit(
                            crate::state::policies::spending_limit_creation_to_policy_state(
                                creation_payload,
                            )?,
                        )
                    }
                    PolicyCreationPayload::SettingsChange(creation_payload) => {
                        PolicyState::SettingsChange(creation_payload.to_policy_state().to_anchor()?)
                    }
                };

                let expiration: Option<PolicyExpiration> =
                    if let Some(expiration_args) = expiration_args {
                        match expiration_args {
                            // Use the provided timestamp
                            PolicyExpirationArgs::Timestamp(timestamp) => {
                                Some(PolicyExpiration::Timestamp(*timestamp))
                            }
                            // Generate the core state hash and use it
                            PolicyExpirationArgs::SettingsState => Some(
                                PolicyExpiration::SettingsState(self.generate_core_state_hash()?),
                            ),
                        }
                    } else {
                        None
                    };

                // Create and serialize the policy
                let policy = Policy::create_state(
                    *self_key,
                    next_policy_seed,
                    policy_bump,
                    signers,
                    *threshold,
                    *time_lock,
                    policy_state,
                    // If no start was submitted, use the current timestamp
                    start_timestamp.unwrap_or(Clock::get()?.unix_timestamp),
                    expiration.clone(),
                    rent_payer.key(),
                )
                .to_anchor()?;

                // Check the policy invariant
                policy.invariant().to_anchor()?;
                policy.try_serialize(&mut &mut policy_info.data.borrow_mut()[..])?;

                // Log the event
                let event = PolicyEvent {
                    event_type: PolicyEventType::Create,
                    settings_pubkey: self_key.to_owned(),
                    policy_pubkey,
                    policy: Some(policy),
                };
                if let Some(log_authority_info) = log_authority_info {
                    SmartAccountEvent::PolicyEvent(event).log(log_authority_info)?;
                }
            }

            SettingsAction::PolicyUpdate {
                policy: policy_key,
                signers,
                threshold,
                time_lock,
                policy_update_payload,
                expiration_args,
            } => {
                // Find the policy account
                let policy_info = remaining_accounts
                    .iter()
                    .find(|acc| acc.key == policy_key)
                    .ok_or(SmartAccountError::MissingAccount)?;

                // Verify the policy account is writable
                require!(policy_info.is_writable, ErrorCode::AccountNotMutable);

                // Deserialize the policy account and verify it belongs to this
                // settings account
                let mut policy = Account::<Policy>::try_from(policy_info)?;

                require_keys_eq!(
                    policy.settings,
                    self_key.to_owned(),
                    SmartAccountError::InvalidAccount
                );

                // Calculate policy data size based on the creation payload
                let policy_specific_data_size = policy_update_payload.policy_state_size();
                let policy_size = Policy::size(signers.len(), policy_specific_data_size);

                // Get the rent payer and system program
                let rent_payer = rent_payer
                    .as_ref()
                    .ok_or(SmartAccountError::MissingAccount)?;
                let system_program = system_program
                    .as_ref()
                    .ok_or(SmartAccountError::MissingAccount)?;

                // Only accept updates to the same policy type
                let new_policy_state = match (&policy.policy_state, policy_update_payload.clone()) {
                    (
                        PolicyState::InternalFundTransfer(_),
                        PolicyCreationPayload::InternalFundTransfer(creation_payload),
                    ) => PolicyState::InternalFundTransfer(
                        creation_payload.to_policy_state().to_anchor()?,
                    ),
                    (
                        PolicyState::ProgramInteraction(_),
                        PolicyCreationPayload::ProgramInteraction(creation_payload),
                    ) => PolicyState::ProgramInteraction(
                        crate::state::policies::program_interaction_creation_to_policy_state(
                            creation_payload,
                        )?,
                    ),
                    (
                        PolicyState::SpendingLimit(_),
                        PolicyCreationPayload::SpendingLimit(creation_payload),
                    ) => PolicyState::SpendingLimit(
                        crate::state::policies::spending_limit_creation_to_policy_state(
                            creation_payload,
                        )?,
                    ),
                    (
                        PolicyState::SettingsChange(_),
                        PolicyCreationPayload::SettingsChange(creation_payload),
                    ) => {
                        PolicyState::SettingsChange(creation_payload.to_policy_state().to_anchor()?)
                    }
                    (_, _) => {
                        return err!(SmartAccountError::InvalidPolicyPayload);
                    }
                };

                // Determine the new expiration
                let expiration: Option<PolicyExpiration> =
                    if let Some(expiration_args) = expiration_args {
                        match expiration_args {
                            // Use the provided timestamp
                            PolicyExpirationArgs::Timestamp(timestamp) => {
                                Some(PolicyExpiration::Timestamp(*timestamp))
                            }
                            // Generate the core state hash and use it
                            PolicyExpirationArgs::SettingsState => Some(
                                PolicyExpiration::SettingsState(self.generate_core_state_hash()?),
                            ),
                        }
                    } else {
                        None
                    };

                // Update the policy
                policy
                    .update_state(
                        signers,
                        *threshold,
                        *time_lock,
                        new_policy_state,
                        expiration.clone(),
                    )
                    .to_anchor()?;

                // Invalidate prior transaction due to the update
                policy.invalidate_prior_transactions();

                // Check the policy invariant
                policy.invariant().to_anchor()?;

                // Realloc the policy account if needed
                <Policy as crate::state::PolicyExt>::realloc_if_needed(
                    policy_info.clone(),
                    signers.len(),
                    policy_size,
                    Some(rent_payer.to_account_info()),
                    Some(system_program.to_account_info()),
                )?;

                // Exit the policy account
                policy.exit(program_id)?;

                // Log the event
                let event = PolicyEvent {
                    event_type: PolicyEventType::Update,
                    settings_pubkey: self_key.to_owned(),
                    policy_pubkey: *policy_key,
                    policy: Some(policy.clone().into_inner()),
                };
                if let Some(log_authority_info) = log_authority_info {
                    SmartAccountEvent::PolicyEvent(event).log(log_authority_info)?;
                }
            }

            SettingsAction::PolicyRemove { policy: policy_key } => {
                let policy_info = remaining_accounts
                    .iter()
                    .find(|acc| acc.key == policy_key)
                    .ok_or(SmartAccountError::MissingAccount)?;

                let rent_collector = rent_payer
                    .as_ref()
                    .ok_or(SmartAccountError::MissingAccount)?;

                let policy = Account::<Policy>::try_from(policy_info)?;

                // Verify the policy belongs to this settings account
                require_keys_eq!(
                    policy.settings,
                    self_key.to_owned(),
                    SmartAccountError::InvalidAccount
                );
                // Verify the policy rent collector matche the account getting reimbursed
                require_keys_eq!(
                    policy.rent_collector,
                    rent_collector.key(),
                    SmartAccountError::InvalidRentCollector
                );

                policy.close(rent_collector.to_account_info())?;

                // Log the event
                let event = PolicyEvent {
                    event_type: PolicyEventType::Remove,
                    settings_pubkey: self_key.to_owned(),
                    policy_pubkey: *policy_key,
                    policy: None,
                };
                if let Some(log_authority_info) = log_authority_info {
                    SmartAccountEvent::PolicyEvent(event).log(log_authority_info)?;
                }
            }
            _ => {
                return err!(SmartAccountError::InvalidAccount);
            }
        }

        Ok(())
    }
}

// Consensus trait implementation for the foreign Settings type (allowed:
// Consensus is local).
impl Consensus for Settings {
    fn account_type(&self) -> ConsensusAccountType {
        ConsensusAccountType::Settings
    }

    fn check_derivation(&self, key: Pubkey) -> Result<()> {
        let (address, _bump) = Pubkey::find_program_address(
            &[SEED_PREFIX, SEED_SETTINGS, self.seed.to_le_bytes().as_ref()],
            &crate::ID,
        );
        require_keys_eq!(address, key, SmartAccountError::InvalidAccount);
        Ok(())
    }

    /// Settings are always active.
    fn is_active(&self, _accounts: &[AccountInfo]) -> Result<()> {
        Ok(())
    }

    fn signers(&self) -> &[SmartAccountSigner] {
        &self.signers
    }

    fn threshold(&self) -> u16 {
        self.threshold
    }

    fn time_lock(&self) -> u32 {
        self.time_lock
    }

    fn transaction_index(&self) -> u64 {
        self.transaction_index
    }

    fn set_transaction_index(&mut self, transaction_index: u64) -> Result<()> {
        self.transaction_index = transaction_index;
        Ok(())
    }

    fn stale_transaction_index(&self) -> u64 {
        self.stale_transaction_index
    }

    fn invalidate_prior_transactions(&mut self) {
        self.stale_transaction_index = self.transaction_index;
    }

    fn invariant(&self) -> Result<()> {
        self.invariant().to_anchor()
    }
}
