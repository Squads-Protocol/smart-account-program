use solana_pubkey::Pubkey;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct ProgramConfigSetAuthorityArgs {
    pub new_authority: Pubkey,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct ProgramConfigSetSmartAccountCreationFeeArgs {
    pub new_smart_account_creation_fee: u64,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct ProgramConfigSetTreasuryArgs {
    pub new_treasury: Pubkey,
}
