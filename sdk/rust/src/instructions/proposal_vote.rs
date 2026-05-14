#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct VoteOnProposalArgs {
    pub memo: Option<String>,
}

pub enum Vote {
    Approve,
    Reject,
    Cancel,
}
