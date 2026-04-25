#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct ExtendTransactionBufferArgs {
    pub buffer: Vec<u8>,
}
