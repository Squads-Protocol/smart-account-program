#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct LegacySyncTransactionArgs {
    /// The index of the smart account this transaction is for.
    pub account_index: u8,
    /// The number of signers to reach threshold and adequate permissions.
    pub num_signers: u8,
    /// Expected to be serialized as a SmallVec<u8, CompiledInstruction>.
    pub instructions: Vec<u8>,
}
