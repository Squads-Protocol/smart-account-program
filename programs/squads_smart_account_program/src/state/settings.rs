use anchor_lang::prelude::*;
use anchor_lang::solana_program::borsh0_10::get_instance_packed_len;
use borsh::BorshSerialize;
use anchor_lang::system_program;
use solana_program::hash::hash;

use crate::AddSpendingLimitEvent;
use crate::LogAuthorityInfo;
use crate::PolicyEvent;
use crate::PolicyEventType;
use crate::RemoveSpendingLimitEvent;
use crate::SmartAccountEvent;
use crate::{
    errors::*,
    id,
    interface::consensus_trait::{Consensus, ConsensusAccountType},
    state::*,
    state::signer_v2::SmartAccountSignerWrapper,
    utils::*,
    SettingsAction,
};
pub const MAX_TIME_LOCK: u32 = 3 * 30 * 24 * 60 * 60; // 3 months

// Account index constants
// Free accounts: 0-250 (251 accounts)
// Reserved accounts: 251-255 (5 accounts) - bypass index validation
pub const FREE_ACCOUNT_MAX_INDEX: u8 = 250;
pub const RESERVED_ACCOUNT_START: u8 = 251;

enum SignerWrapper {
    V1(LegacySmartAccountSigner),
    V2(SmartAccountSigner),
}

#[account]
pub struct Settings {
    /// An integer that is used seed the settings PDA. Its incremented by 1
    /// inside the program conifg by 1 for each smart account created. This is
    /// to ensure uniqueness of each settings PDA without relying on user input.
    ///
    /// Note: As this represents a DOS vector in the current creation architecture,
    /// account creation will be permissioned until compression is implemented.
    pub seed: u128,
    /// The authority that can change the smart account settings.
    /// This is a very important parameter as this authority can change the signers and threshold.
    ///
    /// The convention is to set this to `Pubkey::default()`.
    /// In this case, the smart account becomes autonomous, so every settings change goes through
    /// the normal process of voting by the signers.
    ///
    /// However, if this parameter is set to any other key, all the setting changes for this smart account settings
    /// will need to be signed by the `settings_authority`. We call such a smart account a "controlled smart account".
    pub settings_authority: Pubkey,
    /// Threshold for signatures.
    pub threshold: u16,
    /// How many seconds must pass between transaction voting settlement and execution.
    pub time_lock: u32,
    /// Last transaction index. 0 means no transactions have been created.
    pub transaction_index: u64,
    /// Last stale transaction index. All transactions up until this index are stale.
    /// This index is updated when smart account settings (signers/threshold/time_lock) change.
    pub stale_transaction_index: u64,
    /// Field reserved for when archival/compression is implemented.
    /// Will be set to Pubkey::default() to mark accounts that should
    /// be eligible for archival before the feature is implemented.
    pub archival_authority: Option<Pubkey>,
    /// Field that will prevent a smart account from being archived immediately after unarchival.
    /// This is to prevent a DOS vector where the archival authority could
    /// constantly unarchive and archive the smart account to prevent it from
    /// being used.
    pub archivable_after: u64,
    /// Bump for the smart account PDA seed.
    pub bump: u8,
    /// Signers attached to the smart account (V1 or V2 format with custom serialization)
    pub signers: SmartAccountSignerWrapper,
    /// Counter for how many sub accounts are in use (improves off-chain indexing)
    pub account_utilization: u8,
    /// Seed used for deterministic policy creation.
    pub policy_seed: Option<u64>,
    // Reserved for future use
    pub _reserved2: u8,
}

impl Settings {
    fn base_size() -> usize {
        8  + // anchor account discriminator
        16 + // seed
        32 + // settings_authority
        2  + // threshold
        4  + // time_lock
        8  + // transaction_index
        8  + // stale_transaction_index
        1  + // archival_authority Option discriminator
        32 + // archival_authority (always 32 bytes, even if None)
        8  + // archivable_after
        1  + // bump
        1  + // sub_account_utilization
        1  + 8 + // policy_seed
        1 // _reserved_2
    }

    /// Generates a hash of the core settings: Signers, threshold, and time_lock
    pub fn generate_core_state_hash(&self) -> Result<[u8; 32]> {
        let mut data = Vec::new();
        self.signers
            .serialize(&mut data)
            .map_err(|_| SmartAccountError::SerializationFailed)?;
        data.extend_from_slice(&self.threshold.to_le_bytes());
        data.extend_from_slice(&self.time_lock.to_le_bytes());

        Ok(hash(&data).to_bytes())
    }
    pub fn find_and_initialize_settings_account<'info>(
        &self,
        settings_account_key: Pubkey,
        rent_payer: &AccountInfo<'info>,
        remaining_accounts: &'info [AccountInfo<'info>],
        system_program: &Program<'info, System>,
    ) -> Result<&AccountInfo<'info>> {
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
            SmartAccountError::InvalidAccount
        );

        let rent = Rent::get()?;

        create_account(
            rent_payer,
            settings_account_info,
            system_program,
            &crate::ID,
            &rent,
            Settings::size_for_wrapper(&self.signers),
            vec![
                SEED_PREFIX.to_vec(),
                SEED_SETTINGS.to_vec(),
                self.seed.to_le_bytes().to_vec(),
                vec![self.bump],
            ],
        )?;

        Ok(settings_account_info)
    }

    /// Calculate size for V1 format (fixed 33-byte signers)
    pub fn size(signers_length: usize) -> usize {
        8  + // anchor account discriminator
        16 + // seed
        32 + // settings_authority
        2  + // threshold
        4  + // time_lock
        8  + // transaction_index
        8  + // stale_transaction_index
        1  + // archival_authority Option discriminator
        32 + // archival_authority (always 32 bytes, even if None, just to keep the realloc logic simpler)
        8  + // archivable_after
        1  + // bump
        4  + // signers vector length
        signers_length * LegacySmartAccountSigner::INIT_SPACE + // signers
        1  + // sub_account_utilization
        1  + 8 + // policy_seed
        1 // _reserved_2
    }

    /// Calculate size based on actual wrapper contents (V1 or V2)
    pub fn size_for_wrapper(wrapper: &SmartAccountSignerWrapper) -> usize {
        Self::base_size() + wrapper.serialized_size()
    }

    /// Check if the settings account space needs to be reallocated to accommodate `signers_length`.
    /// Returns `true` if the account was reallocated.
    pub fn realloc_if_needed<'a>(
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

    /// Check if the settings account space needs to be reallocated to accommodate the wrapper.
    /// Returns `true` if the account was reallocated.
    pub fn realloc_if_needed_for_wrapper<'a>(
        settings: AccountInfo<'a>,
        signers_wrapper: &SmartAccountSignerWrapper,
        rent_payer: Option<AccountInfo<'a>>,
        system_program: Option<AccountInfo<'a>>,
    ) -> Result<bool> {
        require_keys_eq!(
            *settings.owner,
            id(),
            SmartAccountError::IllegalAccountOwner
        );

        let current_account_size = settings.data.borrow().len();
        let account_size_to_fit_signers = Settings::size_for_wrapper(signers_wrapper);

        if current_account_size >= account_size_to_fit_signers {
            return Ok(false);
        }

        realloc(
            &settings,
            account_size_to_fit_signers,
            rent_payer,
            system_program,
        )?;

        Ok(true)
    }

    // Makes sure the settings state is valid.
    // This must be called at the end of every instruction that modifies a Settings account.
    pub fn invariant(&self) -> Result<()> {
        let Self {
            threshold,
            signers,
            transaction_index,
            stale_transaction_index,
            ..
        } = self;
        // Max number of signers is u16::MAX.
        require!(
            signers.len() <= usize::from(u16::MAX),
            SmartAccountError::TooManySigners
        );

        // There must be no duplicate signers.
        require!(!signers.has_duplicates(), SmartAccountError::DuplicateSigner);

        // signers must not have unknown permissions.
        require!(
            signers.all_permissions_valid(), // mask < 8 = Initiate | Vote | Execute
            SmartAccountError::UnknownPermission
        );

        // There must be at least one signer with Initiate permission.
        let num_proposers = Self::num_proposers(&self);
        require!(num_proposers > 0, SmartAccountError::NoProposers);

        // There must be at least one signer with Execute permission.
        let num_executors = Self::num_executors(&self);
        require!(num_executors > 0, SmartAccountError::NoExecutors);

        // There must be at least one signer with Vote permission.
        let num_voters = Self::num_voters(&self);
        require!(num_voters > 0, SmartAccountError::NoVoters);

        // Threshold must be greater than 0.
        require!(*threshold > 0, SmartAccountError::InvalidThreshold);

        // Threshold must not exceed the number of voters.
        require!(
            usize::from(*threshold) <= num_voters,
            SmartAccountError::InvalidThreshold
        );

        // `state.stale_transaction_index` must be less than or equal to `state.transaction_index`.
        require!(
            stale_transaction_index <= transaction_index,
            SmartAccountError::InvalidStaleTransactionIndex
        );

        // Time Lock must not exceed the maximum allowed to prevent bricking the settings.
        require!(
            self.time_lock <= MAX_TIME_LOCK,
            SmartAccountError::TimeLockExceedsMaxAllowed
        );

        Ok(())
    }

    pub fn add_signer_v2_checked(&mut self, new_signer: &SmartAccountSigner) -> Result<()> {
        require!(
            self.signers.version() == SIGNERS_VERSION_V2,
            SmartAccountError::MustMigrateToV2
        );

        require!(
            self.signers.len() < MAX_SIGNERS,
            SmartAccountError::MaxSignersReached
        );

        require!(
            self.is_signer(new_signer.key()).is_none(),
            SmartAccountError::DuplicateSigner
        );

        require!(
            !self.signers.has_duplicate_public_key(new_signer),
            SmartAccountError::DuplicatePublicKey
        );

        self.signers.add_signer_v2(new_signer.clone());
        self.signers.sort_by_signer_key();
        self.invalidate_prior_transactions();

        Ok(())
    }

    /// Add `new_signer` to the settings `signers` vec and sort the vec.
    pub fn add_signer(&mut self, new_signer: LegacySmartAccountSigner) {
        self.signers.add_signer(new_signer);
        self.signers.sort_by_signer_key();
    }

    pub fn migrate_signers_wrapper(signers: &SmartAccountSignerWrapper) -> SmartAccountSignerWrapper {
        SmartAccountSignerWrapper::from_v2_signers(signers.as_v2())
    }

    /// Remove `signer_pubkey` from the settings `signers` vec.
    ///
    /// # Errors
    /// - `SmartAccountError::NotASigner` if `signer_pubkey` is not a signer.
    pub fn remove_signer(&mut self, signer_pubkey: Pubkey) -> Result<()> {
        match self.signers.remove_signer(&signer_pubkey) {
            Some(_) => Ok(()),
            None => err!(SmartAccountError::NotASigner),
        }
    }
    // Modify the settings with a given action.
    pub fn modify_with_action<'info>(
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
                self.apply_add_signer_inner(SignerWrapper::V1(new_signer.clone()))?;
            }

            SettingsAction::RemoveSigner { old_signer } => {
                self.apply_remove_signer_inner(*old_signer)?;
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
                self.handle_add_spending_limit(
                    self_key,
                    seed,
                    *account_index,
                    signers,
                    mint,
                    *amount,
                    *period,
                    destinations,
                    *expiration,
                    rent,
                    rent_payer,
                    system_program,
                    remaining_accounts,
                    program_id,
                    log_authority_info,
                )?;
            }

            SettingsAction::RemoveSpendingLimit {
                spending_limit: spending_limit_key,
            } => {
                self.handle_remove_spending_limit(
                    self_key,
                    spending_limit_key,
                    rent_payer,
                    remaining_accounts,
                    log_authority_info,
                )?;
            }

            SettingsAction::SetArchivalAuthority {
                new_archival_authority: _,
            } => {
                // Marked as NotImplemented until archival feature is implemented.
                return err!(SmartAccountError::NotImplemented);
            }

            SettingsAction::PolicyCreate {
                seed,
                policy_creation_payload,
                signers,
                threshold,
                time_lock,
                start_timestamp,
                expiration_args,
            } => {
                self.handle_policy_create(
                    self_key,
                    policy_creation_payload,
                    signers,
                    *threshold,
                    *time_lock,
                    *start_timestamp,
                    expiration_args,
                    rent,
                    rent_payer,
                    system_program,
                    remaining_accounts,
                    program_id,
                    log_authority_info,
                )?;
            }

            SettingsAction::PolicyUpdate {
                policy: policy_key,
                signers,
                threshold,
                time_lock,
                policy_update_payload,
                expiration_args,
            } => {
                self.handle_policy_update(
                    self_key,
                    policy_key,
                    signers,
                    *threshold,
                    *time_lock,
                    policy_update_payload,
                    expiration_args,
                    rent_payer,
                    system_program,
                    remaining_accounts,
                    program_id,
                    log_authority_info,
                )?;
            }

            SettingsAction::PolicyRemove { policy: policy_key } => {
                self.handle_policy_remove(
                    self_key,
                    policy_key,
                    rent_payer,
                    remaining_accounts,
                    log_authority_info,
                )?;
            }

            SettingsAction::PolicyMigrateSigners { policy: policy_key } => {
                self.handle_policy_migrate_signers(
                    self_key,
                    policy_key,
                    rent_payer,
                    system_program,
                    remaining_accounts,
                    log_authority_info,
                )?;
            }

            SettingsAction::AddSignerV2 { new_signer } => {
                self.apply_add_signer_inner(SignerWrapper::V2(new_signer.clone()))?;

                // Realloc handled by caller after modify_with_action returns
            }

            SettingsAction::RemoveSignerV2 { old_signer } => {
                self.apply_remove_signer_inner(*old_signer)?;
            }

            SettingsAction::PolicyCreateV2 {
                seed: _,
                policy_creation_payload,
                signers,
                threshold,
                time_lock,
                start_timestamp,
                expiration_args,
            } => {
                // TODO: Deduplicate PolicyCreate/PolicyUpdate V1/V2 branches (shared flow + signer wrapper).
                // Same as PolicyCreate but with SmartAccountSigner (V2) format
                // Validate that all account indices used by the policy are unlocked
                self.handle_policy_create_v2(
                    self_key,
                    policy_creation_payload,
                    signers,
                    *threshold,
                    *time_lock,
                    *start_timestamp,
                    expiration_args,
                    rent_payer,
                    system_program,
                    remaining_accounts,
                    program_id,
                    log_authority_info,
                )?;
            }

            SettingsAction::PolicyUpdateV2 {
                policy: policy_key,
                signers,
                threshold,
                time_lock,
                policy_update_payload,
                expiration_args,
            } => {
                self.handle_policy_update_v2(
                    self_key,
                    policy_key,
                    signers,
                    *threshold,
                    *time_lock,
                    policy_update_payload,
                    expiration_args,
                    rent_payer,
                    system_program,
                    remaining_accounts,
                    program_id,
                    log_authority_info,
                )?;
            }

            SettingsAction::SetSessionKey {
                signer_key,
                session_key,
                expiration,
            } => {
                self.handle_set_session_key(*signer_key, *session_key, *expiration)?;
            }

            SettingsAction::ClearSessionKey { signer_key } => {
                self.handle_clear_session_key(*signer_key)?;
            }
        }

        Ok(())
    }

    #[inline(never)]
    fn handle_add_spending_limit<'info>(
        &self,
        self_key: &Pubkey,
        seed: &Pubkey,
        account_index: u8,
        signers: &Vec<Pubkey>,
        mint: &Pubkey,
        amount: u64,
        period: Period,
        destinations: &Vec<Pubkey>,
        expiration: i64,
        rent: &Rent,
        rent_payer: &Option<Signer<'info>>,
        system_program: &Option<Program<'info, System>>,
        remaining_accounts: &'info [AccountInfo<'info>],
        program_id: &Pubkey,
        log_authority_info: Option<&LogAuthorityInfo<'info>>,
    ) -> Result<()> {
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
            &spending_limit_info,
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
            account_index,
            signers,
            amount,
            mint: *mint,
            period,
            remaining_amount: amount,
            last_reset: Clock::get()?.unix_timestamp,
            bump: spending_limit_bump,
            destinations: destinations.to_vec(),
            expiration,
        };

        spending_limit.invariant()?;
        spending_limit
            .try_serialize(&mut &mut spending_limit_info.data.borrow_mut()[..])?;

        let event = AddSpendingLimitEvent {
            settings_pubkey: self_key.to_owned(),
            spending_limit_pubkey: spending_limit_key,
            spending_limit: spending_limit.clone(),
        };
        if let Some(log_authority_info) = log_authority_info {
            SmartAccountEvent::AddSpendingLimitEvent(event).log(&log_authority_info)?;
        }

        Ok(())
    }

    #[inline(never)]
    fn handle_remove_spending_limit<'info>(
        &self,
        self_key: &Pubkey,
        spending_limit_key: &Pubkey,
        rent_payer: &Option<Signer<'info>>,
        remaining_accounts: &'info [AccountInfo<'info>],
        log_authority_info: Option<&LogAuthorityInfo<'info>>,
    ) -> Result<()> {
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

        let event = RemoveSpendingLimitEvent {
            settings_pubkey: self_key.to_owned(),
            spending_limit_pubkey: *spending_limit_key,
        };
        if let Some(log_authority_info) = log_authority_info {
            SmartAccountEvent::RemoveSpendingLimitEvent(event).log(&log_authority_info)?;
        }

        Ok(())
    }

    #[inline(never)]
    fn next_policy_seed(&mut self) -> u64 {
        if let Some(policy_seed) = self.policy_seed {
            let next_policy_seed = policy_seed.checked_add(1).unwrap();
            self.policy_seed = Some(next_policy_seed);
            next_policy_seed
        } else {
            self.policy_seed = Some(1);
            1
        }
    }

    fn resolve_policy_expiration(
        &self,
        expiration_args: &Option<PolicyExpirationArgs>,
    ) -> Result<Option<PolicyExpiration>> {
        if let Some(expiration_args) = expiration_args {
            match expiration_args {
                PolicyExpirationArgs::Timestamp(timestamp) => {
                    Ok(Some(PolicyExpiration::Timestamp(*timestamp)))
                }
                PolicyExpirationArgs::SettingsState => {
                    Ok(Some(PolicyExpiration::SettingsState(
                        self.generate_core_state_hash()?,
                    )))
                }
            }
        } else {
            Ok(None)
        }
    }

    fn build_policy_state_for_create_v1(
        &self,
        policy_creation_payload: &PolicyCreationPayload,
    ) -> Result<PolicyState> {
        match policy_creation_payload.clone() {
            PolicyCreationPayload::InternalFundTransfer(creation_payload) => {
                Ok(PolicyState::InternalFundTransfer(
                    creation_payload.to_policy_state()?,
                ))
            }
            PolicyCreationPayload::LegacyProgramInteraction(creation_payload) => {
                Ok(PolicyState::ProgramInteraction(
                    creation_payload.to_policy_state()?,
                ))
            }
            PolicyCreationPayload::ProgramInteraction(creation_payload) => {
                Ok(PolicyState::ProgramInteraction(
                    creation_payload.to_policy_state()?,
                ))
            }
            PolicyCreationPayload::SpendingLimit(mut creation_payload) => {
                let current_timestamp = Clock::get()?.unix_timestamp;
                if creation_payload.time_constraints.accumulate_unused
                    && creation_payload.time_constraints.start < current_timestamp
                {
                    creation_payload.time_constraints.start = current_timestamp;
                }
                Ok(PolicyState::SpendingLimit(
                    creation_payload.to_policy_state()?,
                ))
            }
            PolicyCreationPayload::SettingsChange(creation_payload) => {
                Ok(PolicyState::SettingsChange(creation_payload.to_policy_state()?))
            }
        }
    }

    fn build_policy_state_for_create_v2(
        &self,
        policy_creation_payload: &PolicyCreationPayload,
    ) -> Result<PolicyState> {
        match policy_creation_payload.clone() {
            PolicyCreationPayload::InternalFundTransfer(creation_payload) => {
                Ok(PolicyState::InternalFundTransfer(
                    creation_payload.to_policy_state()?,
                ))
            }
            PolicyCreationPayload::LegacyProgramInteraction(creation_payload) => {
                Ok(PolicyState::ProgramInteraction(
                    creation_payload.to_policy_state()?,
                ))
            }
            PolicyCreationPayload::ProgramInteraction(creation_payload) => {
                Ok(PolicyState::ProgramInteraction(
                    creation_payload.to_policy_state()?,
                ))
            }
            PolicyCreationPayload::SpendingLimit(mut creation_payload) => {
                if creation_payload.time_constraints.start == 0 {
                    let current_timestamp = Clock::get()?.unix_timestamp;
                    creation_payload.time_constraints.start = current_timestamp;
                }
                Ok(PolicyState::SpendingLimit(
                    creation_payload.to_policy_state()?,
                ))
            }
            PolicyCreationPayload::SettingsChange(creation_payload) => {
                Ok(PolicyState::SettingsChange(creation_payload.to_policy_state()?))
            }
        }
    }

    fn build_policy_state_for_update(
        &self,
        policy: &Policy,
        policy_update_payload: &PolicyCreationPayload,
    ) -> Result<PolicyState> {
        match (&policy.policy_state, policy_update_payload.clone()) {
            (
                PolicyState::InternalFundTransfer(_),
                PolicyCreationPayload::InternalFundTransfer(creation_payload),
            ) => Ok(PolicyState::InternalFundTransfer(
                creation_payload.to_policy_state()?,
            )),
            (
                PolicyState::ProgramInteraction(_),
                PolicyCreationPayload::ProgramInteraction(creation_payload),
            ) => Ok(PolicyState::ProgramInteraction(
                creation_payload.to_policy_state()?,
            )),
            (
                PolicyState::SpendingLimit(_),
                PolicyCreationPayload::SpendingLimit(creation_payload),
            ) => Ok(PolicyState::SpendingLimit(
                creation_payload.to_policy_state()?,
            )),
            (
                PolicyState::SettingsChange(_),
                PolicyCreationPayload::SettingsChange(creation_payload),
            ) => Ok(PolicyState::SettingsChange(creation_payload.to_policy_state()?)),
            (_, _) => err!(SmartAccountError::InvalidPolicyPayload),
        }
    }

    fn build_policy_from_wrapper(
        &self,
        self_key: &Pubkey,
        seed: u64,
        bump: u8,
        signers_wrapper: SmartAccountSignerWrapper,
        v1_signers: Option<Vec<LegacySmartAccountSigner>>,
        threshold: u16,
        time_lock: u32,
        policy_state: PolicyState,
        start_timestamp: Option<i64>,
        expiration: Option<PolicyExpiration>,
        rent_collector: Pubkey,
    ) -> Result<Policy> {
        let start = start_timestamp.unwrap_or(Clock::get()?.unix_timestamp);

        if let Some(v1_signers) = v1_signers {
            Policy::create_state(
                *self_key,
                seed,
                bump,
                &v1_signers,
                threshold,
                time_lock,
                policy_state,
                start,
                expiration,
                rent_collector,
            )
        } else {
            let mut v2_signers = signers_wrapper.as_v2();
            v2_signers.sort_by_key(|s| s.key());

            Ok(Policy {
                settings: *self_key,
                seed,
                bump,
                transaction_index: 0,
                stale_transaction_index: 0,
                signers: SmartAccountSignerWrapper::from_v2_signers(v2_signers),
                threshold,
                time_lock,
                policy_state,
                start,
                expiration,
                rent_collector,
            })
        }
    }

    fn apply_add_signer_inner(&mut self, signer: SignerWrapper) -> Result<()> {
        match signer {
            SignerWrapper::V1(signer) => {
                self.add_signer(signer);
                self.invalidate_prior_transactions();
            }
            SignerWrapper::V2(signer) => {
                self.add_signer_v2_checked(&signer)?;
            }
        }

        Ok(())
    }

    fn apply_remove_signer_inner(&mut self, signer_key: Pubkey) -> Result<()> {
        require!(
            self.signers.len() > 1,
            SmartAccountError::RemoveLastSigner
        );

        self.remove_signer(signer_key)?;
        self.invalidate_prior_transactions();

        Ok(())
    }

    #[inline(never)]
    fn handle_policy_create<'info>(
        &mut self,
        self_key: &Pubkey,
        policy_creation_payload: &PolicyCreationPayload,
        signers: &Vec<LegacySmartAccountSigner>,
        threshold: u16,
        time_lock: u32,
        start_timestamp: Option<i64>,
        expiration_args: &Option<PolicyExpirationArgs>,
        rent: &Rent,
        rent_payer: &Option<Signer<'info>>,
        system_program: &Option<Program<'info, System>>,
        remaining_accounts: &'info [AccountInfo<'info>],
        program_id: &Pubkey,
        log_authority_info: Option<&LogAuthorityInfo<'info>>,
    ) -> Result<()> {
        policy_creation_payload.validate_account_indices(self)?;

        let next_policy_seed = self.next_policy_seed();
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

        let policy_specific_data_size = policy_creation_payload.policy_state_size();
        let policy_size = Policy::size(signers.len(), policy_specific_data_size);

        let rent_payer = rent_payer
            .as_ref()
            .ok_or(SmartAccountError::MissingAccount)?;
        let system_program = system_program
            .as_ref()
            .ok_or(SmartAccountError::MissingAccount)?;

        create_account(
            &rent_payer.to_account_info(),
            &policy_info,
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

        let policy_state = self.build_policy_state_for_create_v1(policy_creation_payload)?;
        let expiration = self.resolve_policy_expiration(expiration_args)?;

        let policy = self.build_policy_from_wrapper(
            self_key,
            next_policy_seed,
            policy_bump,
            SmartAccountSignerWrapper::from_v1_signers(signers.clone()),
            Some(signers.clone()),
            threshold,
            time_lock,
            policy_state,
            start_timestamp,
            expiration.clone(),
            rent_payer.key(),
        )?;

        policy.invariant()?;
        policy.try_serialize(&mut &mut policy_info.data.borrow_mut()[..])?;

        let event = PolicyEvent {
            event_type: PolicyEventType::Create,
            settings_pubkey: self_key.to_owned(),
            policy_pubkey,
            policy: Some(policy),
        };
        if let Some(log_authority_info) = log_authority_info {
            SmartAccountEvent::PolicyEvent(event).log(&log_authority_info)?;
        }

        Ok(())
    }

    #[inline(never)]
    fn handle_policy_update<'info>(
        &mut self,
        self_key: &Pubkey,
        policy_key: &Pubkey,
        signers: &Vec<LegacySmartAccountSigner>,
        threshold: u16,
        time_lock: u32,
        policy_update_payload: &PolicyCreationPayload,
        expiration_args: &Option<PolicyExpirationArgs>,
        rent_payer: &Option<Signer<'info>>,
        system_program: &Option<Program<'info, System>>,
        remaining_accounts: &'info [AccountInfo<'info>],
        program_id: &Pubkey,
        log_authority_info: Option<&LogAuthorityInfo<'info>>,
    ) -> Result<()> {
        policy_update_payload.validate_account_indices(self)?;

        let policy_info = remaining_accounts
            .iter()
            .find(|acc| acc.key == policy_key)
            .ok_or(SmartAccountError::MissingAccount)?;

        require!(policy_info.is_writable, SmartAccountError::InvalidAccount);

        let mut policy = Account::<Policy>::try_from(policy_info)?;

        require_keys_eq!(
            policy.settings,
            self_key.to_owned(),
            SmartAccountError::InvalidAccount
        );

        let policy_specific_data_size = policy_update_payload.policy_state_size();
        let policy_size = Policy::size(signers.len(), policy_specific_data_size);

        let rent_payer = rent_payer
            .as_ref()
            .ok_or(SmartAccountError::MissingAccount)?;
        let system_program = system_program
            .as_ref()
            .ok_or(SmartAccountError::MissingAccount)?;

        let new_policy_state = self.build_policy_state_for_update(&policy, policy_update_payload)?;
        let expiration = self.resolve_policy_expiration(expiration_args)?;

        policy.update_state(signers, threshold, time_lock, new_policy_state, expiration.clone())?;

        policy.invalidate_prior_transactions();
        policy.invariant()?;

        Policy::realloc_if_needed(
            policy_info.clone(),
            signers.len(),
            policy_size,
            Some(rent_payer.to_account_info()),
            Some(system_program.to_account_info()),
        )?;

        policy.exit(program_id)?;

        let event = PolicyEvent {
            event_type: PolicyEventType::Update,
            settings_pubkey: self_key.to_owned(),
            policy_pubkey: *policy_key,
            policy: Some(policy.clone().into_inner()),
        };
        if let Some(log_authority_info) = log_authority_info {
            SmartAccountEvent::PolicyEvent(event).log(&log_authority_info)?;
        }

        Ok(())
    }

    #[inline(never)]
    fn handle_policy_migrate_signers<'info>(
        &mut self,
        self_key: &Pubkey,
        policy_key: &Pubkey,
        rent_payer: &Option<Signer<'info>>,
        system_program: &Option<Program<'info, System>>,
        remaining_accounts: &'info [AccountInfo<'info>],
        log_authority_info: Option<&LogAuthorityInfo<'info>>,
    ) -> Result<()> {
        let policy_info = remaining_accounts
            .iter()
            .find(|acc| acc.key == policy_key)
            .ok_or(SmartAccountError::MissingAccount)?
            .clone();

        require!(policy_info.is_writable, SmartAccountError::InvalidAccount);

        let (policy, policy_state_size) = {
            let policy_data = policy_info.try_borrow_data()?;
            let policy = Policy::try_deserialize(&mut &policy_data[..])?;

            require_keys_eq!(
                policy.settings,
                self_key.to_owned(),
                SmartAccountError::InvalidAccount
            );

            require!(
                policy.signers.version() == SIGNERS_VERSION_V1,
                SmartAccountError::AlreadyMigrated
            );

            let policy_state_size = get_instance_packed_len(&policy.policy_state)
                .map_err(|_| SmartAccountError::InvalidPayload)?;
            (policy, policy_state_size)
        };

        let mut migrated_policy = policy.clone();
        migrated_policy.signers = Self::migrate_signers_wrapper(&policy.signers);

        Policy::realloc_for_wrapper(
            policy_info.clone(),
            &migrated_policy.signers,
            policy_state_size,
            rent_payer.as_ref().map(|s| s.to_account_info()),
            system_program.as_ref().map(|p| p.to_account_info()),
        )?;

        let mut policy_data = policy_info.try_borrow_mut_data()?;
        migrated_policy.try_serialize(&mut &mut policy_data[..])?;

        let event = PolicyEvent {
            event_type: PolicyEventType::MigrateSigners,
            settings_pubkey: self_key.to_owned(),
            policy_pubkey: *policy_key,
            policy: Some(migrated_policy),
        };
        if let Some(log_authority_info) = log_authority_info {
            SmartAccountEvent::PolicyEvent(event).log(&log_authority_info)?;
        }

        Ok(())
    }

    #[inline(never)]
    fn handle_policy_create_v2<'info>(
        &mut self,
        self_key: &Pubkey,
        policy_creation_payload: &PolicyCreationPayload,
        signers: &Vec<SmartAccountSigner>,
        threshold: u16,
        time_lock: u32,
        start_timestamp: Option<i64>,
        expiration_args: &Option<PolicyExpirationArgs>,
        rent_payer: &Option<Signer<'info>>,
        system_program: &Option<Program<'info, System>>,
        remaining_accounts: &'info [AccountInfo<'info>],
        program_id: &Pubkey,
        log_authority_info: Option<&LogAuthorityInfo<'info>>,
    ) -> Result<()> {
        policy_creation_payload.validate_account_indices(self)?;

        let next_policy_seed = self.next_policy_seed();
        let (policy_pubkey, policy_bump) = Pubkey::find_program_address(
            &[
                crate::SEED_PREFIX,
                SEED_POLICY,
                self_key.as_ref(),
                next_policy_seed.to_le_bytes().as_ref(),
            ],
            program_id,
        );

        let policy_info = remaining_accounts
            .iter()
            .find(|acc| acc.key == &policy_pubkey)
            .ok_or(SmartAccountError::MissingAccount)?;

        require!(policy_info.data_is_empty(), SmartAccountError::AccountNotEmpty);

        let rent_payer = rent_payer
            .as_ref()
            .ok_or(SmartAccountError::MissingAccount)?;
        let system_program = system_program
            .as_ref()
            .ok_or(SmartAccountError::MissingAccount)?;

        let policy_state = self.build_policy_state_for_create_v2(policy_creation_payload)?;
        let policy_data_size = policy_creation_payload.policy_state_size();

        let signers_wrapper = SmartAccountSignerWrapper::from_v2_signers(signers.clone());
        let policy_size = Policy::size_for_wrapper(&signers_wrapper, policy_data_size);
        let rent = Rent::get()?;

        create_account(
            rent_payer,
            policy_info,
            system_program,
            &crate::ID,
            &rent,
            policy_size,
            vec![
                crate::SEED_PREFIX.to_vec(),
                SEED_POLICY.to_vec(),
                self_key.to_bytes().to_vec(),
                next_policy_seed.to_le_bytes().to_vec(),
                vec![policy_bump],
            ],
        )?;

        let expiration = self.resolve_policy_expiration(expiration_args)?;

        let v1_signers: Vec<LegacySmartAccountSigner> =
            signers.iter().filter_map(|s| s.to_v1()).collect();
        let v1_signers = if v1_signers.len() == signers.len() {
            Some(v1_signers)
        } else {
            None
        };

        let policy = self.build_policy_from_wrapper(
            self_key,
            next_policy_seed,
            policy_bump,
            signers_wrapper.clone(),
            v1_signers,
            threshold,
            time_lock,
            policy_state,
            start_timestamp,
            expiration.clone(),
            rent_payer.key(),
        )?;

        policy.invariant()?;
        policy.try_serialize(&mut &mut policy_info.data.borrow_mut()[..])?;

        let event = PolicyEvent {
            event_type: PolicyEventType::Create,
            settings_pubkey: self_key.to_owned(),
            policy_pubkey,
            policy: Some(policy),
        };
        if let Some(log_authority_info) = log_authority_info {
            SmartAccountEvent::PolicyEvent(event).log(&log_authority_info)?;
        }

        Ok(())
    }

    #[inline(never)]
    fn handle_policy_update_v2<'info>(
        &mut self,
        self_key: &Pubkey,
        policy_key: &Pubkey,
        signers: &Vec<SmartAccountSigner>,
        threshold: u16,
        time_lock: u32,
        policy_update_payload: &PolicyCreationPayload,
        expiration_args: &Option<PolicyExpirationArgs>,
        rent_payer: &Option<Signer<'info>>,
        system_program: &Option<Program<'info, System>>,
        remaining_accounts: &'info [AccountInfo<'info>],
        program_id: &Pubkey,
        log_authority_info: Option<&LogAuthorityInfo<'info>>,
    ) -> Result<()> {
        policy_update_payload.validate_account_indices(self)?;

        let policy_info = remaining_accounts
            .iter()
            .find(|acc| acc.key == policy_key)
            .ok_or(SmartAccountError::MissingAccount)?;

        require!(policy_info.is_writable, SmartAccountError::InvalidAccount);

        let mut policy = Account::<Policy>::try_from(policy_info)?;

        require_keys_eq!(
            policy.settings,
            self_key.to_owned(),
            SmartAccountError::InvalidAccount
        );

        let rent_payer = rent_payer
            .as_ref()
            .ok_or(SmartAccountError::MissingAccount)?;
        let system_program = system_program
            .as_ref()
            .ok_or(SmartAccountError::MissingAccount)?;

        let new_policy_state = self.build_policy_state_for_update(&policy, policy_update_payload)?;
        let expiration = self.resolve_policy_expiration(expiration_args)?;

        let mut sorted_signers = signers.clone();
        sorted_signers.sort_by_key(|s| s.key());

        policy.signers = SmartAccountSignerWrapper::from_v2_signers(sorted_signers);
        policy.threshold = threshold;
        policy.time_lock = time_lock;
        policy.policy_state = new_policy_state;
        policy.expiration = expiration.clone();

        policy.invalidate_prior_transactions();
        policy.invariant()?;

        let policy_data_size = policy_update_payload.policy_state_size();

        Policy::realloc_for_wrapper(
            policy_info.clone(),
            &policy.signers,
            policy_data_size,
            Some(rent_payer.to_account_info()),
            Some(system_program.to_account_info()),
        )?;

        policy.exit(program_id)?;

        let event = PolicyEvent {
            event_type: PolicyEventType::Update,
            settings_pubkey: self_key.to_owned(),
            policy_pubkey: *policy_key,
            policy: Some(policy.clone().into_inner()),
        };
        if let Some(log_authority_info) = log_authority_info {
            SmartAccountEvent::PolicyEvent(event).log(&log_authority_info)?;
        }

        Ok(())
    }

    #[inline(never)]
    fn handle_policy_remove<'info>(
        &self,
        self_key: &Pubkey,
        policy_key: &Pubkey,
        rent_payer: &Option<Signer<'info>>,
        remaining_accounts: &'info [AccountInfo<'info>],
        log_authority_info: Option<&LogAuthorityInfo<'info>>,
    ) -> Result<()> {
        let policy_info = remaining_accounts
            .iter()
            .find(|acc| acc.key == policy_key)
            .ok_or(SmartAccountError::MissingAccount)?;

        let rent_collector = rent_payer
            .as_ref()
            .ok_or(SmartAccountError::MissingAccount)?;

        let policy = Account::<Policy>::try_from(policy_info)?;

        require_keys_eq!(
            policy.settings,
            self_key.to_owned(),
            SmartAccountError::InvalidAccount
        );
        require_keys_eq!(
            policy.rent_collector,
            rent_collector.key(),
            SmartAccountError::InvalidRentCollector
        );

        policy.close(rent_collector.to_account_info())?;

        let event = PolicyEvent {
            event_type: PolicyEventType::Remove,
            settings_pubkey: self_key.to_owned(),
            policy_pubkey: *policy_key,
            policy: None,
        };
        if let Some(log_authority_info) = log_authority_info {
            SmartAccountEvent::PolicyEvent(event).log(&log_authority_info)?;
        }

        Ok(())
    }

    #[inline(never)]
    fn handle_set_session_key(
        &mut self,
        signer_key: Pubkey,
        session_key: Pubkey,
        expiration: u64,
    ) -> Result<()> {
        require!(
            self.signers.version() == SIGNERS_VERSION_V2,
            SmartAccountError::MustMigrateToV2
        );

        let signer_index = self
            .signers
            .find_index(&signer_key)
            .ok_or(SmartAccountError::NotASigner)?;

        let current_timestamp = Clock::get()?.unix_timestamp as u64;

        match &mut self.signers {
            SmartAccountSignerWrapper::V2(signers) => {
                let signer = signers
                    .get_mut(signer_index)
                    .ok_or(SmartAccountError::NotASigner)?;

                require!(signer.is_external(), SmartAccountError::InvalidSignerType);

                signer.set_session_key(session_key, expiration, current_timestamp)?;
            }
            SmartAccountSignerWrapper::V1(_) => {
                return Err(SmartAccountError::MustMigrateToV2.into());
            }
        }

        Ok(())
    }

    #[inline(never)]
    fn handle_clear_session_key(&mut self, signer_key: Pubkey) -> Result<()> {
        require!(
            self.signers.version() == SIGNERS_VERSION_V2,
            SmartAccountError::MustMigrateToV2
        );

        let signer_index = self
            .signers
            .find_index(&signer_key)
            .ok_or(SmartAccountError::NotASigner)?;

        match &mut self.signers {
            SmartAccountSignerWrapper::V2(signers) => {
                let signer = signers
                    .get_mut(signer_index)
                    .ok_or(SmartAccountError::NotASigner)?;

                require!(signer.is_external(), SmartAccountError::InvalidSignerType);

                signer.clear_session_key()?;
            }
            SmartAccountSignerWrapper::V1(_) => {
                return Err(SmartAccountError::MustMigrateToV2.into());
            }
        }

        Ok(())
    }

    pub fn increment_account_utilization(&mut self) {
        self.account_utilization = self.account_utilization.checked_add(1).unwrap();
    }

    /// Validates that the given account index is unlocked.
    /// Reserved accounts (251-255) bypass this check.
    pub fn validate_account_index_unlocked(&self, index: u8) -> Result<()> {
        // Reserved accounts (251-255) bypass the check
        if index >= RESERVED_ACCOUNT_START {
            return Ok(());
        }
        require!(
            index <= self.account_utilization,
            SmartAccountError::AccountIndexLocked
        );
        Ok(())
    }

    /// Validates that all given account indices are unlocked.
    pub fn validate_account_indices_unlocked(&self, indices: &[u8]) -> Result<()> {
        for index in indices {
            self.validate_account_index_unlocked(*index)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_signers_wrapper_preserves_signers() {
        let v1_signers = vec![
            LegacySmartAccountSigner {
                key: Pubkey::new_unique(),
                permissions: Permissions::all(),
            },
            LegacySmartAccountSigner {
                key: Pubkey::new_unique(),
                permissions: Permissions { mask: 0b011 },
            },
        ];
        let wrapper = SmartAccountSignerWrapper::from_v1_signers(v1_signers.clone());

        let migrated = Settings::migrate_signers_wrapper(&wrapper);
        assert_eq!(migrated.version(), SIGNERS_VERSION_V2);
        assert_eq!(migrated.len(), v1_signers.len());

        let migrated_signers = migrated.as_v2();
        for (expected, actual) in v1_signers.iter().zip(migrated_signers.iter()) {
            assert_eq!(expected.key, actual.key());
            assert_eq!(expected.permissions, actual.permissions());
        }
    }
}

#[derive(AnchorDeserialize, AnchorSerialize, InitSpace, Eq, PartialEq, Clone, Debug)]
pub struct LegacySmartAccountSigner {
    pub key: Pubkey,
    pub permissions: Permissions,
}

#[derive(Clone, Copy)]
pub enum Permission {
    Initiate = 1 << 0,
    Vote = 1 << 1,
    Execute = 1 << 2,
}

/// Bitmask for permissions.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Eq, PartialEq, Clone, Copy, Default, Debug,
)]
pub struct Permissions {
    pub mask: u8,
}

impl Permissions {
    /// Currently unused.
    pub fn from_vec(permissions: &[Permission]) -> Self {
        let mut mask = 0;
        for permission in permissions {
            mask |= *permission as u8;
        }
        Self { mask }
    }

    pub fn has(&self, permission: Permission) -> bool {
        self.mask & (permission as u8) != 0
    }

    pub fn all() -> Self {
        Self { mask: 0b111 }
    }
}

// Implement Consensus for Settings
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

    /// Settings are always active, and don't have an expiration.
    fn is_active(&self, _accounts: &[AccountInfo]) -> Result<()> {
        Ok(())
    }

    fn signers(&self) -> &SmartAccountSignerWrapper {
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
        self.invariant()
    }

    fn apply_counter_updates(&mut self, updates: &[(Pubkey, u64)]) -> Result<()> {
        self.signers.apply_counter_updates(updates)
    }
}
