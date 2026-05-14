#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Debug)]
pub struct LogEventArgs {
    pub account_seeds: Vec<Vec<u8>>,
    pub bump: u8,
    pub event: Vec<u8>,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Debug)]
pub struct LogEventArgsV2 {
    pub event: Vec<u8>,
}
