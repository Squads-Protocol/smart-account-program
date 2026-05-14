use solana_address::Address;

use crate::state::SmartAccountSigner;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct CreateSmartAccountArgs {
    pub settings_authority: Option<Address>,
    pub threshold: u16,
    pub signers: Vec<SmartAccountSigner>,
    pub time_lock: u32,
    pub rent_collector: Option<Address>,
    pub memo: Option<String>,
}
