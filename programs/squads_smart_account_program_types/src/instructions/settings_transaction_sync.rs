use crate::state::SettingsAction;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct SyncSettingsTransactionArgs {
    pub num_signers: u8,
    pub actions: Vec<SettingsAction>,
    pub memo: Option<String>,
}
