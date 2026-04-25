//! Thin wrapper over the types-crate `derive_ephemeral_signers` that passes
//! this program's ID automatically. Preserves the existing two-arg call sites.

use anchor_lang::prelude::Pubkey;

/// Return a tuple of ephemeral_signer_keys and ephemeral_signer_seeds derived
/// from the given `ephemeral_signer_bumps` and `transaction_key`.
pub fn derive_ephemeral_signers(
    transaction_key: Pubkey,
    ephemeral_signer_bumps: &[u8],
) -> (Vec<Pubkey>, Vec<Vec<Vec<u8>>>) {
    squads_smart_account_program_types::derive_ephemeral_signers(
        &crate::ID,
        transaction_key,
        ephemeral_signer_bumps,
    )
}
