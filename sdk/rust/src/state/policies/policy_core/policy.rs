use solana_address::Address;

use super::payloads::PolicyPayload;
use crate::errors::SmartAccountError;
use crate::state::policies::implementations::{
    InternalFundTransferPolicy, ProgramInteractionPolicy, SettingsChangePolicy, SpendingLimitPolicy,
};
use crate::state::{MAX_TIME_LOCK, Permission, SmartAccountSigner};

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyExpiration {
    /// Policy expires at a specific timestamp.
    Timestamp(i64),
    /// Policy expires when the core settings hash mismatches the stored hash.
    SettingsState([u8; 32]),
}

impl PolicyExpiration {
    // Variant payloads are `i64` (8) and `[u8; 32]` (32). So `1 + 32 = 33`.
    pub const INIT_SPACE: usize = 1 + 32;
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyExpirationArgs {
    /// Policy expires at a specific timestamp.
    Timestamp(i64),
    /// Policy expires when the core settings hash mismatches the stored hash.
    SettingsState,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub struct Policy {
    /// The smart account this policy belongs to.
    pub settings: Address,
    /// The seed of the policy.
    pub seed: u64,
    /// Bump for the policy.
    pub bump: u8,
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
    /// Rent Collector for the policy for when it gets closed.
    pub rent_collector: Address,
}

impl Policy {
    pub const DISCRIMINATOR: [u8; 8] = [0xde, 0x87, 0x07, 0xa3, 0xeb, 0xb1, 0x21, 0x44];

    pub fn size(signers_length: usize, policy_data_length: usize) -> usize {
        8  + // anchor discriminator
        32 + // settings
        8  + // seed
        1  + // bump
        8  + // transaction_index
        8  + // stale_transaction_index
        4  + // signers vector length
        signers_length * SmartAccountSigner::INIT_SPACE + // signers
        2  + // threshold
        4  + // time_lock
        1  + policy_data_length + // discriminator + policy_data_length
        8  + // start_timestamp
        1  + PolicyExpiration::INIT_SPACE + // expiration (discriminator + max data size)
        32 // rent_collector
    }

    pub fn invariant(&self) -> Result<(), SmartAccountError> {
        if self.signers.len() > usize::from(u16::MAX) {
            return Err(SmartAccountError::TooManySigners);
        }

        let has_duplicates = self.signers.windows(2).any(|win| win[0].key == win[1].key);
        if has_duplicates {
            return Err(SmartAccountError::DuplicateSigner);
        }

        if !self.signers.iter().all(|s| s.permissions.mask < 8) {
            return Err(SmartAccountError::UnknownPermission);
        }

        if self.num_proposers() == 0 {
            return Err(SmartAccountError::NoProposers);
        }
        if self.num_executors() == 0 {
            return Err(SmartAccountError::NoExecutors);
        }
        if self.num_voters() == 0 {
            return Err(SmartAccountError::NoVoters);
        }

        if self.threshold == 0 {
            return Err(SmartAccountError::InvalidThreshold);
        }
        if usize::from(self.threshold) > self.num_voters() {
            return Err(SmartAccountError::InvalidThreshold);
        }

        if self.stale_transaction_index > self.transaction_index {
            return Err(SmartAccountError::InvalidStaleTransactionIndex);
        }

        if let Some(PolicyExpiration::Timestamp(timestamp)) = &self.expiration
            && *timestamp <= self.start
        {
            return Err(SmartAccountError::PolicyInvariantInvalidExpiration);
        }

        if self.time_lock > MAX_TIME_LOCK {
            return Err(SmartAccountError::TimeLockExceedsMaxAllowed);
        }
        self.policy_state.invariant()?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    /// Create policy state safely.
    pub fn create_state(
        settings: Address,
        seed: u64,
        bump: u8,
        signers: &[SmartAccountSigner],
        threshold: u16,
        time_lock: u32,
        policy_state: PolicyState,
        start: i64,
        expiration: Option<PolicyExpiration>,
        rent_collector: Address,
    ) -> Result<Policy, SmartAccountError> {
        let mut sorted_signers = signers.to_vec();
        sorted_signers.sort_by_key(|s| s.key);

        Ok(Policy {
            settings,
            seed,
            bump,
            transaction_index: 0,
            stale_transaction_index: 0,
            signers: sorted_signers,
            threshold,
            time_lock,
            policy_state,
            start,
            expiration,
            rent_collector,
        })
    }

    /// Update policy state safely.
    pub fn update_state(
        &mut self,
        signers: &[SmartAccountSigner],
        threshold: u16,
        time_lock: u32,
        policy_state: PolicyState,
        expiration: Option<PolicyExpiration>,
    ) -> Result<(), SmartAccountError> {
        let mut sorted_signers = signers.to_vec();
        sorted_signers.sort_by_key(|s| s.key);

        self.signers = sorted_signers;
        self.threshold = threshold;
        self.time_lock = time_lock;
        self.policy_state = policy_state;
        self.expiration = expiration;
        Ok(())
    }

    pub fn is_signer(&self, signer_pubkey: Address) -> Option<usize> {
        self.signers
            .binary_search_by_key(&signer_pubkey, |s| s.key)
            .ok()
    }

    pub fn signer_has_permission(&self, signer_pubkey: Address, permission: Permission) -> bool {
        match self.is_signer(signer_pubkey) {
            Some(index) => self.signers[index].permissions.has(permission),
            _ => false,
        }
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

    pub fn cutoff(&self) -> usize {
        self.num_voters()
            .checked_sub(usize::from(self.threshold))
            .unwrap()
            .checked_add(1)
            .unwrap()
    }

    pub fn invalidate_prior_transactions(&mut self) {
        self.stale_transaction_index = self.transaction_index;
    }
}

#[cfg(feature = "borsh")]
impl Policy {
    pub fn try_deserialize(data: &[u8]) -> Result<Self, borsh::io::Error> {
        if data.len() < 8 || data[..8] != Self::DISCRIMINATOR {
            return Err(borsh::io::Error::new(
                borsh::io::ErrorKind::InvalidData,
                "discriminator mismatch",
            ));
        }
        let mut body = &data[8..];
        <Self as borsh::BorshDeserialize>::deserialize(&mut body)
    }
}

// Silence unused PolicyPayload import warning when `borsh` is off.
#[allow(dead_code)]
fn _policy_payload_ref(_: &PolicyPayload) {}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Debug)]
pub enum PolicyState {
    /// Internal fund transfer policy.
    InternalFundTransfer(InternalFundTransferPolicy),
    /// Spending limit policy.
    SpendingLimit(SpendingLimitPolicy),
    /// Settings change policy.
    SettingsChange(SettingsChangePolicy),
    /// Program interaction policy.
    ProgramInteraction(ProgramInteractionPolicy),
}

impl PolicyState {
    pub fn invariant(&self) -> Result<(), SmartAccountError> {
        match self {
            PolicyState::InternalFundTransfer(policy) => policy.invariant(),
            PolicyState::SpendingLimit(policy) => policy.invariant(),
            PolicyState::SettingsChange(policy) => policy.invariant(),
            PolicyState::ProgramInteraction(policy) => policy.invariant(),
        }
    }
}
