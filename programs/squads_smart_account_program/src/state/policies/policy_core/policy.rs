//! `Policy` — re-exported from the types crate plus a `PolicyExt` extension
//! trait for realloc and payload dispatch methods that depend on anchor / CPI.

use anchor_lang::prelude::*;

pub use squads_smart_account_program_types::{PolicyExpiration, PolicyExpirationArgs, PolicyState};
pub use squads_smart_account_program_types::Policy;

use super::payloads::PolicyPayload;
use super::traits::PolicyExecutionContext;
use crate::error_conv::ToAnchorResult;
use crate::{
    errors::*,
    interface::consensus_trait::{Consensus, ConsensusAccountType},
    InternalFundTransferExecutionArgs, ProgramInteractionExecutionArgs,
    Proposal, Settings, SettingsChangeExecutionArgs, SmartAccountSigner,
    SpendingLimitExecutionArgs, Transaction, SEED_POLICY, SEED_PREFIX,
};

/// Program-side extensions for `Policy`: methods needing Clock, realloc, CPI.
pub trait PolicyExt {
    fn realloc_if_needed<'a>(
        policy: AccountInfo<'a>,
        signers_length: usize,
        policy_data_length: usize,
        rent_payer: Option<AccountInfo<'a>>,
        system_program: Option<AccountInfo<'a>>,
    ) -> Result<bool>;

    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &PolicyPayload,
    ) -> Result<()>;

    fn execute<'info>(
        &mut self,
        transaction_account: Option<&Account<'info, Transaction>>,
        proposal_account: Option<&Account<'info, Proposal>>,
        payload: &PolicyPayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()>;
}

impl PolicyExt for Policy {
    /// Check if the policy account space needs to be reallocated.
    fn realloc_if_needed<'a>(
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

    /// Dispatch validation to the matching policy-specific implementation.
    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &PolicyPayload,
    ) -> Result<()> {
        use crate::state::policies::implementations::{
            InternalFundTransferPolicyExt, SpendingLimitPolicyExt, SettingsChangePolicyExt,
            ProgramInteractionPolicyExt,
        };
        match (&self.policy_state, payload) {
            (
                PolicyState::InternalFundTransfer(policy),
                PolicyPayload::InternalFundTransfer(payload),
            ) => InternalFundTransferPolicyExt::validate_payload(policy, context, payload),
            (PolicyState::SpendingLimit(policy), PolicyPayload::SpendingLimit(payload)) => {
                SpendingLimitPolicyExt::validate_payload(policy, context, payload)
            }
            (PolicyState::SettingsChange(policy), PolicyPayload::SettingsChange(payload)) => {
                SettingsChangePolicyExt::validate_payload(policy, context, payload)
            }
            (
                PolicyState::ProgramInteraction(policy),
                PolicyPayload::ProgramInteraction(payload),
            ) => ProgramInteractionPolicyExt::validate_payload(policy, context, payload),
            _ => err!(SmartAccountError::InvalidPolicyPayload),
        }
    }

    /// Dispatch execution to the matching policy-specific implementation.
    fn execute<'info>(
        &mut self,
        transaction_account: Option<&Account<'info, Transaction>>,
        proposal_account: Option<&Account<'info, Proposal>>,
        payload: &PolicyPayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        use crate::state::policies::policy_core::traits::PolicyTrait;
        match (&mut self.policy_state, payload) {
            (
                PolicyState::InternalFundTransfer(ref mut policy_state),
                PolicyPayload::InternalFundTransfer(payload),
            ) => {
                let args = InternalFundTransferExecutionArgs {
                    settings_key: self.settings,
                };
                policy_state.execute_payload(args, payload, accounts)
            }
            (
                PolicyState::SpendingLimit(ref mut policy_state),
                PolicyPayload::SpendingLimit(payload),
            ) => {
                let args = SpendingLimitExecutionArgs {
                    settings_key: self.settings,
                };
                policy_state.execute_payload(args, payload, accounts)
            }
            (
                PolicyState::ProgramInteraction(ref mut policy_state),
                PolicyPayload::ProgramInteraction(payload),
            ) => {
                let args = ProgramInteractionExecutionArgs {
                    settings_key: self.settings,
                    // if the transaction account is not provided, use a default
                    // pubkey (sync transactions)
                    transaction_key: transaction_account
                        .map(|t| t.key())
                        .unwrap_or(Pubkey::default()),
                    // if the proposal account is not provided, use a default
                    // pubkey (sync transactions)
                    proposal_key: proposal_account
                        .map(|p| p.key())
                        .unwrap_or(Pubkey::default()),
                    policy_signers: self.signers.clone(),
                };
                policy_state.execute_payload(args, payload, accounts)
            }
            (
                PolicyState::SettingsChange(ref mut policy_state),
                PolicyPayload::SettingsChange(payload),
            ) => {
                let args = SettingsChangeExecutionArgs {
                    settings_key: self.settings,
                };
                policy_state.execute_payload(args, payload, accounts)
            }
            _ => err!(SmartAccountError::InvalidPolicyPayload),
        }
    }
}

// Consensus impl for the foreign Policy type — local trait, so OK.
impl Consensus for Policy {
    /// Checks if a given policy is active based on it's start and expiration
    fn is_active(&self, accounts: &[AccountInfo]) -> Result<()> {
        // Get the current timestamp
        let current_timestamp = Clock::get()?.unix_timestamp;
        // Check if the policy has started
        require!(
            current_timestamp >= self.start,
            SmartAccountError::PolicyNotActiveYet
        );
        // Check if the policy is expired
        match self.expiration {
            Some(PolicyExpiration::Timestamp(expiration_timestamp)) => {
                // Get current timestamp
                let current_timestamp = Clock::get()?.unix_timestamp;
                require!(
                    current_timestamp < expiration_timestamp,
                    SmartAccountError::PolicyExpirationViolationTimestampExpired
                );
                Ok(())
            }
            Some(PolicyExpiration::SettingsState(stored_hash)) => {
                // Find the settings account in the accounts list
                let settings_account_info = accounts
                    .first()
                    .ok_or(SmartAccountError::PolicyExpirationViolationSettingsAccountNotPresent)?;
                require!(
                    settings_account_info.key() == self.settings,
                    SmartAccountError::PolicyExpirationViolationPolicySettingsKeyMismatch
                );
                // Deserialize the settings account
                let account_data = settings_account_info.try_borrow_data()?;
                let settings = Settings::try_deserialize(&mut &**account_data)?;

                // Generate the current core state hash
                let current_hash = crate::state::SettingsExt::generate_core_state_hash(&settings)?;

                require!(
                    current_hash == stored_hash,
                    SmartAccountError::PolicyExpirationViolationHashExpired
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
            &[
                SEED_PREFIX,
                SEED_POLICY,
                self.settings.as_ref(),
                self.seed.to_le_bytes().as_ref(),
            ],
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

    fn invariant(&self) -> Result<()> {
        self.invariant().to_anchor()
    }
}
