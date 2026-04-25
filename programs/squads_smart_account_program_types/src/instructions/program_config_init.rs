use solana_program::pubkey::Pubkey;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct InitProgramConfigArgs {
    pub authority: Pubkey,
    pub smart_account_creation_fee: u64,
    pub treasury: Pubkey,
}
