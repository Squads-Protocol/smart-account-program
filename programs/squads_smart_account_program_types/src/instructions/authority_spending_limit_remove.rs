#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct RemoveSpendingLimitArgs {
    pub memo: Option<String>,
}
