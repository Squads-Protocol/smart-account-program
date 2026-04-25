#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub struct UseSpendingLimitArgs {
    pub amount: u64,
    pub decimals: u8,
    pub memo: Option<String>,
}
