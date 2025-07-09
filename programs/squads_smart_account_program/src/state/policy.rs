use anchor_lang::prelude::*;

use crate::{
    errors::*,
    interface::consensus_trait::{Consensus, ConsensusAccountType, ConsensusSigner},
    SEED_POLICY, SEED_PREFIX,
};

#[account]
pub struct Policy {
    /// The smart account this policy belongs to.
    pub settings: Pubkey,

    /// Transaction index for stale transaction protection.
    pub transaction_index: u64,

    /// Stale transaction index boundary.
    pub stale_transaction_index: u64,

    /// Signers attached to the policy with their permissions.
    pub signers: Vec<PolicySigner>,

    /// Threshold for approvals.
    pub threshold: u16,

    /// How many seconds must pass between approval and execution.
    pub time_lock: u32,

    /// The type of policy this represents.
    pub policy_type: PolicyType,

    /// Serialized policy-specific configuration data.
    pub policy_data: Vec<u8>,

    /// Which vault indices this policy applies to.
    /// Empty means applies to all vaults.
    pub vault_scopes: Vec<u8>,
}

impl Policy {
    pub fn size(
        signers_length: usize,
        policy_data_length: usize,
        vault_scopes_length: usize,
    ) -> usize {
        8  + // anchor discriminator
        32 + // smart_account
        8  + // transaction_index
        8  + // stale_transaction_index
        4  + // signers vector length
        signers_length * PolicySigner::INIT_SPACE + // signers
        2  + // threshold
        4  + // time_lock
        1  + // policy_type
        4  + // policy_data vector length
        policy_data_length + // policy_data
        4  + // vault_scopes vector length
        vault_scopes_length + // vault_scopes
        1  + // bump
        8  + // created_at
        8 // updated_at
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
        vault_scopes_length: usize,
        rent_payer: Option<AccountInfo<'a>>,
        system_program: Option<AccountInfo<'a>>,
    ) -> Result<bool> {
        let current_account_size = policy.data.borrow().len();
        let required_size = Policy::size(signers_length, policy_data_length, vault_scopes_length);

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
    pub fn add_signer(&mut self, new_signer: PolicySigner) {
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

#[derive(AnchorDeserialize, AnchorSerialize, InitSpace, Eq, PartialEq, Clone)]
pub struct PolicySigner {
    pub key: Pubkey,
    pub permissions: Permissions,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyType {
    /// Token transfer spending limits (current spending limits functionality).
    SpendingLimit,
    /// Constrained program interactions with validation rules.
    ProgramInteraction,
}

// Re-export Permission and Permissions from settings.rs to maintain consistency
pub use crate::state::settings::{Permission, Permissions};

// Implement ConsensusSigner for PolicySigner
impl ConsensusSigner for PolicySigner {
    fn key(&self) -> Pubkey {
        self.key
    }

    fn permissions(&self) -> Permissions {
        self.permissions
    }
}

// Implement Consensus for Policy
impl Consensus for Policy {
    type SignerType = PolicySigner;

    fn account_type(&self) -> ConsensusAccountType {
        ConsensusAccountType::Policy
    }

    fn key(&self) -> Pubkey {
        // This will be set by Anchor when the account is loaded
        Pubkey::default() // Placeholder - will be overridden by account context
    }

    fn check_derivation(&self) -> Result<()> {
        // TODO: Since policies can be closed, we need to make the derivation deterministic.
        let (address, _bump) = Pubkey::find_program_address(
            &[SEED_PREFIX, SEED_POLICY, self.settings.as_ref()],
            &crate::ID,
        );
        require_keys_eq!(address, self.key(), SmartAccountError::InvalidAccount);
        Ok(())
    }
    fn signers(&self) -> &[Self::SignerType] {
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
