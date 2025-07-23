use anchor_lang::prelude::*;

use crate::{
    errors::*,
    interface::consensus_trait::{Consensus, ConsensusAccountType},
    InternalFundTransferExecutionArgs, Permission, ProgramInteractionExecutionArgs,
    ProgramInteractionPolicy, Proposal, Settings, SettingsChangePolicy, SmartAccountSigner,
    SpendingLimitExecutionArgs, SpendingLimitPolicy, Transaction, SEED_POLICY, SEED_PREFIX,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub enum PolicyExpiration {
    /// Policy expires at a specific timestamp
    Timestamp(i64),
    /// Policy expires when the core settings hash mismatches the stored hash.
    SettingsState([u8; 32]),
}

use super::{payloads::PolicyPayload, traits::PolicyTrait};
use crate::state::policies::implementations::InternalFundTransferPolicy;

#[account]
pub struct Policy {
    /// The smart account this policy belongs to.
    pub settings: Pubkey,

    /// The seed of the policy.
    pub seed: u64,

    /// Transaction index for stale transaction protection.
    pub transaction_index: u64,

    /// Stale transaction index boundary.
    pub stale_transaction_index: u64,

    /// Signers attached to the policy with their permissions.
    pub signers: Vec<SmartAccountSigner>,

    /// Threshold for approvals.
    pub threshold: u16,

    /// How many seconds must pass between approval and execution.
    pub time_lock: u32,

    /// The state of the policy.
    pub policy_state: PolicyState,

    /// Timestamp when the policy becomes active.
    pub start: i64,

    /// Policy expiration - either time-based or state-based.
    pub expiration: Option<PolicyExpiration>,
}

impl Policy {
    pub fn size(signers_length: usize, policy_data_length: usize) -> usize {
        8  + // anchor discriminator
        32 + // settings
        8  + // seed
        8  + // transaction_index
        8  + // stale_transaction_index
        4  + // signers vector length
        signers_length * SmartAccountSigner::INIT_SPACE + // signers
        2  + // threshold
        4  + // time_lock
        1  + // policy_type discriminator
        policy_data_length + // policy_type data
        8  + // start_timestamp
        1  + 32 // expiration (discriminator + max data size)
    }

    pub fn num_voters(&self) -> usize {
        self.signers
            .iter()
            .filter(|s| s.permissions.has(Permission::Vote))
            .count()
    }

    pub fn num_proposers(&self) -> usize {
        self.signers
            .iter()
            .filter(|s| s.permissions.has(Permission::Initiate))
            .count()
    }

    pub fn num_executors(&self) -> usize {
        self.signers
            .iter()
            .filter(|s| s.permissions.has(Permission::Execute))
            .count()
    }

    /// Check if the policy account space needs to be reallocated.
    pub fn realloc_if_needed<'a>(
        policy: AccountInfo<'a>,
        signers_length: usize,
        policy_data_length: usize,
        rent_payer: Option<AccountInfo<'a>>,
        system_program: Option<AccountInfo<'a>>,
    ) -> Result<bool> {
        let current_account_size = policy.data.borrow().len();
        let required_size = Policy::size(signers_length, policy_data_length);

        if current_account_size >= required_size {
            return Ok(false);
        }

        crate::utils::realloc(&policy, required_size, rent_payer, system_program)?;
        Ok(true)
    }

    pub fn invariant(&self) -> Result<()> {
        // Max number of signers is u16::MAX.
        require!(
            self.signers.len() <= usize::from(u16::MAX),
            SmartAccountError::TooManySigners
        );

        // There must be no duplicate signers.
        let has_duplicates = self.signers.windows(2).any(|win| win[0].key == win[1].key);
        require!(!has_duplicates, SmartAccountError::DuplicateSigner);

        // Signers must not have unknown permissions.
        require!(
            self.signers.iter().all(|s| s.permissions.mask < 8),
            SmartAccountError::UnknownPermission
        );

        // There must be at least one signer with Initiate permission.
        require!(self.num_proposers() > 0, SmartAccountError::NoProposers);

        // There must be at least one signer with Execute permission.
        require!(self.num_executors() > 0, SmartAccountError::NoExecutors);

        // There must be at least one signer with Vote permission.
        require!(self.num_voters() > 0, SmartAccountError::NoVoters);

        // Threshold must be greater than 0.
        require!(self.threshold > 0, SmartAccountError::InvalidThreshold);

        // Threshold must not exceed the number of voters.
        require!(
            usize::from(self.threshold) <= self.num_voters(),
            SmartAccountError::InvalidThreshold
        );

        // Stale transaction index must be <= transaction index.
        require!(
            self.stale_transaction_index <= self.transaction_index,
            SmartAccountError::InvalidStaleTransactionIndex
        );

        // Policy state must be valid as well
        self.policy_state.invariant()?;

        Ok(())
    }

    /// Makes transactions created up until this moment stale.
    pub fn invalidate_prior_transactions(&mut self) {
        self.stale_transaction_index = self.transaction_index;
    }

    /// Returns `Some(index)` if `signer_pubkey` is a signer.
    pub fn is_signer(&self, signer_pubkey: Pubkey) -> Option<usize> {
        self.signers
            .binary_search_by_key(&signer_pubkey, |s| s.key)
            .ok()
    }

    pub fn signer_has_permission(&self, signer_pubkey: Pubkey, permission: Permission) -> bool {
        match self.is_signer(signer_pubkey) {
            Some(index) => self.signers[index].permissions.has(permission),
            _ => false,
        }
    }

    /// How many "reject" votes are needed to make a transaction "Rejected".
    pub fn cutoff(&self) -> usize {
        self.num_voters()
            .checked_sub(usize::from(self.threshold))
            .unwrap()
            .checked_add(1)
            .unwrap()
    }

    /// Add a new signer to the policy and sort the signers vec.
    pub fn add_signer(&mut self, new_signer: SmartAccountSigner) {
        self.signers.push(new_signer);
        self.signers.sort_by_key(|s| s.key);
    }

    /// Remove a signer from the policy.
    pub fn remove_signer(&mut self, signer_pubkey: Pubkey) -> Result<()> {
        let signer_index = match self.is_signer(signer_pubkey) {
            Some(index) => index,
            None => return err!(SmartAccountError::NotASigner),
        };

        self.signers.remove(signer_index);
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub enum PolicyState {
    /// Internal fund transfer policy.
    InternalFundTransfer(InternalFundTransferPolicy),
    /// Spending limit policy
    SpendingLimit(SpendingLimitPolicy),
    /// Settings change policy
    SettingsChange(SettingsChangePolicy),
    /// Program interaction policy
    ProgramInteraction(ProgramInteractionPolicy),
}

impl PolicyState {
    pub fn invariant(&self) -> Result<()> {
        match self {
            PolicyState::InternalFundTransfer(policy) => policy.invariant(),
            PolicyState::SpendingLimit(policy) => policy.invariant(),
            PolicyState::SettingsChange(policy) => policy.invariant(),
            PolicyState::ProgramInteraction(policy) => policy.invariant(),
        }
    }
}

impl Policy {
    /// Validate the payload against the policy.
    pub fn validate_payload(&self, payload: &PolicyPayload) -> Result<()> {
        match (&self.policy_state, payload) {
            (
                PolicyState::InternalFundTransfer(policy),
                PolicyPayload::InternalFundTransfer(payload),
            ) => policy.validate_payload(payload),
            (PolicyState::SpendingLimit(policy), PolicyPayload::SpendingLimit(payload)) => {
                policy.validate_payload(payload)
            }
            (PolicyState::SettingsChange(policy), PolicyPayload::SettingsChange(payload)) => {
                policy.validate_payload(payload)
            }
            (
                PolicyState::ProgramInteraction(policy),
                PolicyPayload::ProgramInteraction(payload),
            ) => policy.validate_payload(payload),
            _ => err!(SmartAccountError::InvalidPolicyPayload),
        }
    }
    /// Dispatch method for policy execution
    pub fn validate_and_execute<'info>(
        &mut self,
        transaction_account: Option<&Account<'info, Transaction>>,
        proposal_account: Option<&Account<'info, Proposal>>,
        payload: &PolicyPayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        match (&mut self.policy_state, payload) {
            (
                PolicyState::InternalFundTransfer(ref mut policy_state),
                PolicyPayload::InternalFundTransfer(payload),
            ) => {
                policy_state.validate_payload(&payload)?;
                let args = InternalFundTransferExecutionArgs {
                    settings_key: self.settings,
                };
                policy_state.execute_payload(args, payload, accounts)
            }
            (
                PolicyState::SpendingLimit(ref mut policy_state),
                PolicyPayload::SpendingLimit(payload),
            ) => {
                policy_state.validate_payload(&payload)?;
                let args = SpendingLimitExecutionArgs {
                    settings_key: self.settings,
                };
                policy_state.execute_payload(args, payload, accounts)
            }
            (
                PolicyState::ProgramInteraction(ref mut policy_state),
                PolicyPayload::ProgramInteraction(payload),
            ) => {
                policy_state.validate_payload(&payload)?;
                match (transaction_account, proposal_account) {
                    (Some(transaction), Some(proposal)) => {
                        let args = ProgramInteractionExecutionArgs {
                            settings_key: self.settings,
                            transaction_key: transaction.key(),
                            proposal_key: proposal.key(),
                        };
                        policy_state.execute_payload(args, payload, accounts)
                    }
                    (None, None) => !unimplemented!(),
                    _ => err!(SmartAccountError::InvalidAccount),
                }
            }
            (
                PolicyState::SettingsChange(ref mut policy_state),
                PolicyPayload::SettingsChange(payload),
            ) => {
                policy_state.validate_payload(&payload)?;
                policy_state.execute_payload((), payload, accounts)
            }
            _ => err!(SmartAccountError::InvalidPolicyPayload),
        }
    }
}
// Implement Consensus for Policy
impl Consensus for Policy {
    fn is_active(&self, accounts: &[AccountInfo]) -> Result<()> {
        // Check if the policy is expired
        match self.expiration {
            Some(PolicyExpiration::Timestamp(expiration_timestamp)) => {
                // Get current timestamp
                let current_timestamp = Clock::get()?.unix_timestamp;
                require!(
                    current_timestamp < expiration_timestamp,
                    SmartAccountError::PolicyExpired
                );
                Ok(())
            }
            Some(PolicyExpiration::SettingsState(stored_hash)) => {
                // Find the settings account in the accounts list
                let settings_account_info = accounts
                    .iter()
                    .find(|acc| acc.key() == self.settings)
                    .ok_or(SmartAccountError::InvalidAccount)?;
                // Deserialize the settings account
                let account_data = settings_account_info.try_borrow_data()?;
                let settings = Settings::try_deserialize(&mut &**account_data)?;
                let current_hash = settings.generate_core_state_hash()?;
                require!(
                    current_hash != stored_hash,
                    SmartAccountError::PolicyExpired
                );
                Ok(())
            }
            // If the policy has no expiration, it is always active
            None => Ok(()),
        }
    }
    fn account_type(&self) -> ConsensusAccountType {
        ConsensusAccountType::Policy
    }

    fn check_derivation(&self, key: Pubkey) -> Result<()> {
        // TODO: Since policies can be closed, we need to make the derivation deterministic.
        let (address, _bump) = Pubkey::find_program_address(
            &[SEED_PREFIX, self.settings.as_ref(), SEED_POLICY, self.seed.to_le_bytes().as_ref()],
            &crate::ID,
        );
        require_keys_eq!(address, key, SmartAccountError::InvalidAccount);
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
