use solana_address::Address;

use crate::state::Period;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct AddSpendingLimitArgs {
    pub seed: Address,
    pub account_index: u8,
    pub mint: Address,
    pub amount: u64,
    pub period: Period,
    pub signers: Vec<Address>,
    pub destinations: Vec<Address>,
    pub expiration: i64,
    pub memo: Option<String>,
}
