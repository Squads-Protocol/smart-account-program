#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct CreateProposalArgs {
    pub transaction_index: u64,
    pub draft: bool,
}
