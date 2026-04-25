use solana_program::pubkey::Pubkey;

use crate::state::SmartAccountSigner;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct AddSignerArgs {
    pub new_signer: SmartAccountSigner,
    /// Memo is used for indexing only.
    pub memo: Option<String>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct RemoveSignerArgs {
    pub old_signer: Pubkey,
    pub memo: Option<String>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct ChangeThresholdArgs {
    pub new_threshold: u16,
    pub memo: Option<String>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct SetTimeLockArgs {
    pub time_lock: u32,
    pub memo: Option<String>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct SetNewSettingsAuthorityArgs {
    pub new_settings_authority: Pubkey,
    pub memo: Option<String>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct SetArchivalAuthorityArgs {
    pub new_archival_authority: Option<Pubkey>,
    pub memo: Option<String>,
}
