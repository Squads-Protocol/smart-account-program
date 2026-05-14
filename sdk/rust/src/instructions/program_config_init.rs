use solana_address::Address;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct InitProgramConfigArgs {
    pub authority: Address,
    pub smart_account_creation_fee: u64,
    pub treasury: Address,
}
