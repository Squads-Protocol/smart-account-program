use solana_pubkey::Pubkey;

use crate::state::Period;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct AddSpendingLimitArgs {
    pub seed: Pubkey,
    pub account_index: u8,
    pub mint: Pubkey,
    pub amount: u64,
    pub period: Period,
    pub signers: Vec<Pubkey>,
    pub destinations: Vec<Pubkey>,
    pub expiration: i64,
    pub memo: Option<String>,
}
