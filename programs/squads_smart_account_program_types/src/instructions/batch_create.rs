#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct CreateBatchArgs {
    pub account_index: u8,
    pub memo: Option<String>,
}
