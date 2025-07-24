use anchor_lang::prelude::*;
use anchor_lang::system_program;
use solana_program::hash::hash;

use crate::{
    errors::*,
    id,
    interface::consensus_trait::{Consensus, ConsensusAccountType},
    state::*,
    utils::*,
    SettingsAction,
};
pub const MAX_TIME_LOCK: u32 = 3 * 30 * 24 * 60 * 60; // 3 months

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
    /// Signers attached to the smart account
    pub signers: Vec<SmartAccountSigner>,
    /// Counter for how many sub accounts are in use (improves off-chain indexing)
    pub account_utilization: u8,
    /// Seed used for deterministic policy creation.
    pub policy_seed: Option<u64>,
    // Reserved for future use
    pub _reserved2: u8,
}

impl Settings {
    /// Generates a hash of the core settings: Signers, threshold, and time_lock
    pub fn generate_core_state_hash(&self) -> Result<[u8; 32]> {
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
        signers_length * SmartAccountSigner::INIT_SPACE + // signers
        1  + // sub_account_utilization
        1  + 8 + // policy_seed
        1 // _reserved_2
    }

    pub fn num_voters(signers: &[SmartAccountSigner]) -> usize {
        signers
            .iter()
            .filter(|m| m.permissions.has(Permission::Vote))
            .count()
    }

    pub fn num_proposers(signers: &[SmartAccountSigner]) -> usize {
        signers
            .iter()
            .filter(|m| m.permissions.has(Permission::Initiate))
            .count()
    }

    pub fn num_executors(signers: &[SmartAccountSigner]) -> usize {
        signers
            .iter()
            .filter(|m| m.permissions.has(Permission::Execute))
            .count()
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
        let has_duplicates = signers.windows(2).any(|win| win[0].key == win[1].key);
        require!(!has_duplicates, SmartAccountError::DuplicateSigner);

        // signers must not have unknown permissions.
        require!(
            signers.iter().all(|m| m.permissions.mask < 8), // 8 = Initiate | Vote | Execute
            SmartAccountError::UnknownPermission
        );

        // There must be at least one signer with Initiate permission.
        let num_proposers = Self::num_proposers(signers);
        require!(num_proposers > 0, SmartAccountError::NoProposers);

        // There must be at least one signer with Execute permission.
        let num_executors = Self::num_executors(signers);
        require!(num_executors > 0, SmartAccountError::NoExecutors);

        // There must be at least one signer with Vote permission.
        let num_voters = Self::num_voters(signers);
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

    /// Makes the transactions created up until this moment stale.
    /// Should be called whenever any settings parameter related to the voting consensus is changed.
    pub fn invalidate_prior_transactions(&mut self) {
        self.stale_transaction_index = self.transaction_index;
    }

    /// Returns `Some(index)` if `signer_pubkey` is a signer, with `index` into the `signers` vec.
    /// `None` otherwise.
    pub fn is_signer(&self, signer_pubkey: Pubkey) -> Option<usize> {
        self.signers
            .binary_search_by_key(&signer_pubkey, |m| m.key)
            .ok()
    }

    pub fn signer_has_permission(&self, signer_pubkey: Pubkey, permission: Permission) -> bool {
        match self.is_signer(signer_pubkey) {
            Some(index) => self.signers[index].permissions.has(permission),
            _ => false,
        }
    }

    /// How many "reject" votes are enough to make the transaction "Rejected".
    /// The cutoff must be such that it is impossible for the remaining voters to reach the approval threshold.
    /// For example: total voters = 7, threshold = 3, cutoff = 5.
    pub fn cutoff(&self) -> usize {
        Self::num_voters(&self.signers)
            .checked_sub(usize::from(self.threshold))
            .unwrap()
            .checked_add(1)
            .unwrap()
    }

    /// Add `new_signer` to the settings `signers` vec and sort the vec.
    pub fn add_signer(&mut self, new_signer: SmartAccountSigner) {
        self.signers.push(new_signer);
        self.signers.sort_by_key(|m| m.key);
    }

    /// Remove `signer_pubkey` from the settings `signers` vec.
    ///
    /// # Errors
    /// - `SmartAccountError::NotASigner` if `signer_pubkey` is not a signer.
    pub fn remove_signer(&mut self, signer_pubkey: Pubkey) -> Result<()> {
        let old_signer_index = match self.is_signer(signer_pubkey) {
            Some(old_signer_index) => old_signer_index,
            None => return err!(SmartAccountError::NotASigner),
        };

        self.signers.remove(old_signer_index);

        Ok(())
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
    ) -> Result<()> {
        match action {
            SettingsAction::AddSigner { new_signer } => {
                self.add_signer(new_signer.to_owned());
                self.invalidate_prior_transactions();
            }

            SettingsAction::RemoveSigner { old_signer } => {
                self.remove_signer(old_signer.to_owned())?;
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

                spending_limit.invariant()?;
                spending_limit
                    .try_serialize(&mut &mut spending_limit_info.data.borrow_mut()[..])?;
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
            }

            SettingsAction::SetArchivalAuthority {
                new_archival_authority,
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
                expiration,
            } => {
                // Increment the policy seed if it exists, otherwise set it to
                // 1 (First policy is being created)
                let next_policy_seed = if let Some(policy_seed) = self.policy_seed {
                    let next_policy_seed = policy_seed.checked_add(1).unwrap();
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
                        self_key.as_ref(),
                        SEED_POLICY,
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
                    &policy_info,
                    &system_program.to_account_info(),
                    &id(),
                    rent,
                    policy_size,
                    vec![
                        crate::SEED_PREFIX.to_vec(),
                        self_key.as_ref().to_vec(),
                        SEED_POLICY.to_vec(),
                        seed.to_le_bytes().to_vec(),
                        vec![policy_bump],
                    ],
                )?;

                // Convert creation payload to policy type
                // TODO: Get rid of this clone
                let policy_state = match policy_creation_payload.clone() {
                    PolicyCreationPayload::InternalFundTransfer(creation_payload) => {
                        PolicyState::InternalFundTransfer(creation_payload.to_policy_state())
                    }
                    PolicyCreationPayload::ProgramInteraction(creation_payload) => {
                        PolicyState::ProgramInteraction(creation_payload.to_policy_state())
                    }
                    PolicyCreationPayload::SpendingLimit(creation_payload) => {
                        PolicyState::SpendingLimit(creation_payload.to_policy_state())
                    }
                    PolicyCreationPayload::SettingsChange(creation_payload) => {
                        PolicyState::SettingsChange(creation_payload.to_policy_state())
                    }
                };

                // Create and serialize the policy
                let policy = Policy {
                    settings: *self_key,
                    seed: next_policy_seed,
                    transaction_index: 0,
                    stale_transaction_index: 0,
                    signers: signers.clone(),
                    threshold: *threshold,
                    time_lock: *time_lock,
                    policy_state: policy_state,
                    start: start_timestamp.unwrap_or(Clock::get()?.unix_timestamp),
                    expiration: expiration.clone(),
                };

                // Check the policy invariant
                policy.invariant()?;
                policy.try_serialize(&mut &mut policy_info.data.borrow_mut()[..])?;
            }

            SettingsAction::PolicyRemove { policy: policy_key } => {
                let policy_info = remaining_accounts
                    .iter()
                    .find(|acc| acc.key == policy_key)
                    .ok_or(SmartAccountError::MissingAccount)?;

                let rent_payer = rent_payer
                    .as_ref()
                    .ok_or(SmartAccountError::MissingAccount)?;

                let policy = Account::<Policy>::try_from(policy_info)?;

                // Verify the policy belongs to this settings account
                require_keys_eq!(
                    policy.settings,
                    self_key.to_owned(),
                    SmartAccountError::InvalidAccount
                );

                policy.close(rent_payer.to_account_info())?;
            }
        }

        Ok(())
    }

    pub fn increment_account_utilization(&mut self) {
        self.account_utilization = self.account_utilization.checked_add(1).unwrap();
    }
}

#[derive(AnchorDeserialize, AnchorSerialize, InitSpace, Eq, PartialEq, Clone)]
pub struct SmartAccountSigner {
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
}
