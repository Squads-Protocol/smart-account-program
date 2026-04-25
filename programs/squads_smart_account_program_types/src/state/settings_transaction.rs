use solana_program::pubkey::Pubkey;

use crate::state::policies::policy_core::PolicyCreationPayload;
use crate::state::{Period, PolicyExpirationArgs, SmartAccountSigner};

/// Stores data required for execution of a settings configuration transaction.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
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
    pub const DISCRIMINATOR: [u8; 8] = [0xc7, 0x97, 0x48, 0x57, 0x4d, 0x7c, 0x10, 0x00];
}

#[cfg(feature = "borsh")]
impl SettingsTransaction {
    pub fn try_deserialize(data: &[u8]) -> Result<Self, borsh::maybestd::io::Error> {
        if data.len() < 8 || data[..8] != Self::DISCRIMINATOR {
            return Err(borsh::maybestd::io::Error::new(
                borsh::maybestd::io::ErrorKind::InvalidData,
                "discriminator mismatch",
            ));
        }
        let mut body = &data[8..];
        <Self as borsh::BorshDeserialize>::deserialize(&mut body)
    }

    pub fn size(actions: &[SettingsAction]) -> Result<usize, borsh::maybestd::io::Error> {
        let actions_size: usize = actions
            .iter()
            .map(|action| borsh::to_vec(action).map(|v| v.len()))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum();

        Ok(8 +   // anchor account discriminator
            32 +  // settings
            32 +  // creator
            32 +  // rent_collector
            8 +   // index
            1 +   // bump
            4 +   // actions vector length
            actions_size)
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
#[non_exhaustive]
pub enum SettingsAction {
    /// Add a new member to the settings.
    AddSigner { new_signer: SmartAccountSigner },
    /// Remove a member from the settings.
    RemoveSigner { old_signer: Pubkey },
    /// Change the `threshold` of the settings.
    ChangeThreshold { new_threshold: u16 },
    /// Change the `time_lock` of the settings.
    SetTimeLock { new_time_lock: u32 },
    /// Create a new spending limit.
    AddSpendingLimit {
        seed: Pubkey,
        account_index: u8,
        mint: Pubkey,
        amount: u64,
        period: Period,
        signers: Vec<Pubkey>,
        destinations: Vec<Pubkey>,
        expiration: i64,
    },
    /// Remove a spending limit from the settings.
    RemoveSpendingLimit { spending_limit: Pubkey },
    /// Set the `archival_authority` config parameter of the settings.
    SetArchivalAuthority {
        new_archival_authority: Option<Pubkey>,
    },
    /// Create a new policy account.
    PolicyCreate {
        seed: u64,
        policy_creation_payload: PolicyCreationPayload,
        signers: Vec<SmartAccountSigner>,
        threshold: u16,
        time_lock: u32,
        start_timestamp: Option<i64>,
        expiration_args: Option<PolicyExpirationArgs>,
    },
    /// Update a policy account.
    PolicyUpdate {
        policy: Pubkey,
        signers: Vec<SmartAccountSigner>,
        threshold: u16,
        time_lock: u32,
        policy_update_payload: PolicyCreationPayload,
        expiration_args: Option<PolicyExpirationArgs>,
    },
    /// Remove a policy account.
    PolicyRemove { policy: Pubkey },
}
