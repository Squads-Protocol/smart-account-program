#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct AddTransactionToBatchArgs {
    pub ephemeral_signers: u8,
    pub transaction_message: Vec<u8>,
}
