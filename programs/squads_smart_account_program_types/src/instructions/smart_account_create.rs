use solana_program::pubkey::Pubkey;

use crate::state::SmartAccountSigner;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct CreateSmartAccountArgs {
    pub settings_authority: Option<Pubkey>,
    pub threshold: u16,
    pub signers: Vec<SmartAccountSigner>,
    pub time_lock: u32,
    pub rent_collector: Option<Pubkey>,
    pub memo: Option<String>,
}
