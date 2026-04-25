#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct CreateTransactionBufferArgs {
    pub buffer_index: u8,
    pub account_index: u8,
    pub final_buffer_hash: [u8; 32],
    pub final_buffer_size: u16,
    pub buffer: Vec<u8>,
}
