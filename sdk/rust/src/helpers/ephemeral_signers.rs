//! Ephemeral-signer PDA derivation. Gated on the `ephemeral-signers` feature
//! because [`Address::create_program_address`] requires the `curve25519`
//! capability on `solana-address`, which the crate enables by default in
//! [`crate::Cargo.toml`] but the helper feature makes the dependency explicit
//! for downstream consumers that build with `--no-default-features`.

use solana_address::Address;

use crate::helpers::pda::{SEED_EPHEMERAL_SIGNER, SEED_PREFIX};

/// Return a tuple of (ephemeral_signer_keys, ephemeral_signer_seeds) derived
/// from the given `ephemeral_signer_bumps` and `transaction_key`.
///
/// The returned seeds include the trailing bump byte so they can be passed
/// directly to `invoke_signed` without further manipulation.
pub fn derive_ephemeral_signers(
    program_id: &Address,
    transaction_key: Address,
    ephemeral_signer_bumps: &[u8],
) -> (Vec<Address>, Vec<Vec<Vec<u8>>>) {
    ephemeral_signer_bumps
        .iter()
        .enumerate()
        .map(|(index, bump)| {
            let seeds = vec![
                SEED_PREFIX.to_vec(),
                transaction_key.to_bytes().to_vec(),
                SEED_EPHEMERAL_SIGNER.to_vec(),
                u8::try_from(index).unwrap().to_le_bytes().to_vec(),
                vec![*bump],
            ];
            let address = Address::create_program_address(
                seeds
                    .iter()
                    .map(Vec::as_slice)
                    .collect::<Vec<&[u8]>>()
                    .as_slice(),
                program_id,
            )
            .unwrap();
            (address, seeds)
        })
        .unzip()
}
