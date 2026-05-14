use crate::state::SettingsAction;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct CreateSettingsTransactionArgs {
    pub actions: Vec<SettingsAction>,
    pub memo: Option<String>,
}
