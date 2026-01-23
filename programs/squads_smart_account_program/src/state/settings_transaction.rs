use anchor_lang::prelude::*;
use anchor_lang::solana_program::borsh0_10::get_instance_packed_len;

use super::*;
use crate::state::policies::policy_core::{PolicyCreationPayload};

/// Stores data required for execution of a settings configuration transaction.
/// Settings transactions can perform a predefined set of actions on the Settings PDA, such as adding/removing members,
/// changing the threshold, etc.
#[account]
pub struct SettingsTransaction {
    /// The settings this belongs to.
    pub settings: Pubkey,
    /// Signer on the settings who submitted the transaction.
    pub creator: Pubkey,
    /// The rent collector for the settings transaction account.
    pub rent_collector: Pubkey,
    /// Index of this transaction within the settings.
    pub index: u64,
    /// bump for the transaction seeds.
    pub bump: u8,
    /// Action to be performed on the settings.
    pub actions: Vec<SettingsAction>,
}

impl SettingsTransaction {
    pub fn size(actions: &[SettingsAction]) -> usize {
        let actions_size: usize = actions
            .iter()
            .map(|action| get_instance_packed_len(action).unwrap())
            .sum();

        8 +   // anchor account discriminator
        32 +  // settings
        32 +  // creator
        32 +  // rent_collector
        8 +   // index
        1 +   // bump
        4 +  // actions vector length
        actions_size
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
#[non_exhaustive]
pub enum SettingsAction {
    /// Add a new member to the settings.
    AddSigner { new_signer: LegacySmartAccountSigner },
    /// Remove a member from the settings.
    RemoveSigner { old_signer: Pubkey },
    /// Change the `threshold` of the settings.
    ChangeThreshold { new_threshold: u16 },
    /// Change the `time_lock` of the settings.
    SetTimeLock { new_time_lock: u32 },
    /// Change the `time_lock` of the settings.
    AddSpendingLimit {
        /// Key that is used to seed the SpendingLimit PDA.
        seed: Pubkey,
        /// The index of the account that the spending limit is for.
        account_index: u8,
        /// The token mint the spending limit is for.
        mint: Pubkey,
        /// The amount of tokens that can be spent in a period.
        /// This amount is in decimals of the mint,
        /// so 1 SOL would be `1_000_000_000` and 1 USDC would be `1_000_000`.
        amount: u64,
        /// The reset period of the spending limit.
        /// When it passes, the remaining amount is reset, unless it's `Period::OneTime`.
        period: Period,
        /// Members of the settings that can use the spending limit.
        /// In case a member is removed from the settings, the spending limit will remain existent
        /// (until explicitly deleted), but the removed member will not be able to use it anymore.
        signers: Vec<Pubkey>,
        /// The destination addresses the spending limit is allowed to sent funds to.
        /// If empty, funds can be sent to any address.
        destinations: Vec<Pubkey>,
        /// The expiration timestamp of the spending limit.
        /// Non expiring spending limits are set to `i64::MAX`.
        expiration: i64,
    },
    /// Remove a spending limit from the settings.
    RemoveSpendingLimit { spending_limit: Pubkey },
    /// Set the `archival_authority` config parameter of the settings.
    SetArchivalAuthority { new_archival_authority: Option<Pubkey> },
    /// Create a new policy account.
    // TODO: Check with Orion if we want to add V2 variants (PolicyCreateV2, PolicyUpdateV2)
    // that take SmartAccountSigner (V2) for external signer support on policies.
    // Currently using LegacySmartAccountSigner for V1 compatibility.
    PolicyCreate {
        /// Key that is used to seed the Policy PDA.
        seed: u64,
        /// The policy creation payload containing policy-specific configuration.
        policy_creation_payload: PolicyCreationPayload,
        /// Signers attached to the policy with their permissions.
        signers: Vec<LegacySmartAccountSigner>,
        /// Threshold for approvals on the policy.
        threshold: u16,
        /// How many seconds must pass between approval and execution.
        time_lock: u32,
        /// Timestamp when the policy becomes active.
        start_timestamp: Option<i64>,
        /// Policy expiration - either time-based or state-based.
        expiration_args: Option<PolicyExpirationArgs>,
    },
    /// Update a policy account.
    PolicyUpdate {
        /// The policy account to update.
        policy: Pubkey,
        /// Signers attached to the policy with their permissions.
        signers: Vec<LegacySmartAccountSigner>,
        /// Threshold for approvals on the policy.
        threshold: u16,
        /// How many seconds must pass between approval and execution.
        time_lock: u32,
        /// The policy update payload containing policy-specific configuration.
        policy_update_payload: PolicyCreationPayload,
        /// Policy expiration - either time-based or state-based.
        expiration_args: Option<PolicyExpirationArgs>,
    },
    /// Remove a policy account.
    PolicyRemove {
        /// The policy account to remove.
        policy: Pubkey
    },
    /// Migrate policy signers from V1 to V2 format.
    /// This enables external signer support on the policy.
    PolicyMigrateSigners {
        /// The policy account to migrate.
        policy: Pubkey,
    },
    /// Add a new V2 signer (Native or External) to the settings.
    /// Requires Settings to be migrated to V2 format first.
    AddSignerV2 { new_signer: SmartAccountSigner },
    /// Remove a V2 signer (Native or External) from the settings.
    /// Works the same as RemoveSigner but uses key/key_id from SmartAccountSigner.
    RemoveSignerV2 { old_signer: Pubkey },
    /// Create a new policy account with V2 signers (supports external signers).
    PolicyCreateV2 {
        /// Key that is used to seed the Policy PDA.
        seed: u64,
        /// The policy creation payload containing policy-specific configuration.
        policy_creation_payload: PolicyCreationPayload,
        /// Signers attached to the policy with their permissions (V2 format).
        signers: Vec<SmartAccountSigner>,
        /// Threshold for approvals on the policy.
        threshold: u16,
        /// How many seconds must pass between approval and execution.
        time_lock: u32,
        /// Timestamp when the policy becomes active.
        start_timestamp: Option<i64>,
        /// Policy expiration - either time-based or state-based.
        expiration_args: Option<PolicyExpirationArgs>,
    },
    /// Update a policy account with V2 signers (supports external signers).
    PolicyUpdateV2 {
        /// The policy account to update.
        policy: Pubkey,
        /// Signers attached to the policy with their permissions (V2 format).
        signers: Vec<SmartAccountSigner>,
        /// Threshold for approvals on the policy.
        threshold: u16,
        /// How many seconds must pass between approval and execution.
        time_lock: u32,
        /// The policy update payload containing policy-specific configuration.
        policy_update_payload: PolicyCreationPayload,
        /// Policy expiration - either time-based or state-based.
        expiration_args: Option<PolicyExpirationArgs>,
    },
    /// Set a session key for an external signer.
    /// Session keys allow temporary native Solana keys to sign on behalf of external signers.
    SetSessionKey {
        /// The key_id of the external signer to set the session key for.
        signer_key: Pubkey,
        /// The new session key (must be a valid Solana pubkey).
        session_key: Pubkey,
        /// Expiration timestamp (unix timestamp in seconds).
        /// Must be in the future and not exceed SESSION_KEY_EXPIRATION_LIMIT (3 months).
        expiration: u64,
    },
    /// Clear/revoke a session key for an external signer.
    ClearSessionKey {
        /// The key_id of the external signer to clear the session key for.
        signer_key: Pubkey,
    },
}
