//! Minimal `Instruction` and `AccountMeta` types that mirror
//! `solana_instruction::{Instruction, AccountMeta}` field-for-field.
//!
//! Defining them here lets the `instructions` feature stay free of any
//! `solana-program` / `solana-instruction` dependency. External clients and
//! SDKs can convert to their own `Instruction` type with a trivial
//! field-by-field copy (the layouts are identical).

use solana_pubkey::Pubkey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    pub fn new(pubkey: Pubkey, is_signer: bool) -> Self {
        Self {
            pubkey,
            is_signer,
            is_writable: true,
        }
    }

    pub fn new_readonly(pubkey: Pubkey, is_signer: bool) -> Self {
        Self {
            pubkey,
            is_signer,
            is_writable: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMeta>,
    pub data: Vec<u8>,
}
