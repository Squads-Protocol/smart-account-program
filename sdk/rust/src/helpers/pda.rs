//! PDA derivation helpers.
//!
//! The codama Rust renderer (3.1.0) doesn't emit `find_*_pda` helpers — these
//! are TS-only. The seeds below come from `codama/transforms/pdas.ts`, which
//! is the single source of truth shared with the TypeScript SDK, so derivation
//! stays consistent across languages.
//!
//! Each `find_*_pda` returns `(Address, bump)` from
//! [`solana_address::Address::find_program_address`]. Pass the bump back to
//! the program when the on-chain handler verifies it, e.g.
//! `account_bump: smart_account_bump` in `CreateTransactionArgs`.

use solana_address::Address;

use crate::SQUADS_SMART_ACCOUNT_PROGRAM_ID;

pub const SEED_PREFIX: &[u8] = b"smart_account";
pub const SEED_PROGRAM_CONFIG: &[u8] = b"program_config";
pub const SEED_SETTINGS: &[u8] = b"settings";
pub const SEED_SMART_ACCOUNT: &[u8] = b"smart_account";
pub const SEED_EPHEMERAL_SIGNER: &[u8] = b"ephemeral_signer";
pub const SEED_TRANSACTION: &[u8] = b"transaction";
pub const SEED_PROPOSAL: &[u8] = b"proposal";
pub const SEED_BATCH_TRANSACTION: &[u8] = b"batch_transaction";
pub const SEED_SPENDING_LIMIT: &[u8] = b"spending_limit";
pub const SEED_POLICY: &[u8] = b"policy";
pub const SEED_TRANSACTION_BUFFER: &[u8] = b"transaction_buffer";

pub fn find_program_config_pda() -> (Address, u8) {
    Address::find_program_address(
        &[SEED_PREFIX, SEED_PROGRAM_CONFIG],
        &SQUADS_SMART_ACCOUNT_PROGRAM_ID,
    )
}

pub fn find_settings_pda(account_index: u128) -> (Address, u8) {
    let index_bytes = account_index.to_le_bytes();
    Address::find_program_address(
        &[SEED_PREFIX, SEED_SETTINGS, &index_bytes],
        &SQUADS_SMART_ACCOUNT_PROGRAM_ID,
    )
}

pub fn find_smart_account_pda(settings: &Address, account_index: u8) -> (Address, u8) {
    let index_bytes = account_index.to_le_bytes();
    Address::find_program_address(
        &[
            SEED_PREFIX,
            settings.as_ref(),
            SEED_SMART_ACCOUNT,
            &index_bytes,
        ],
        &SQUADS_SMART_ACCOUNT_PROGRAM_ID,
    )
}

pub fn find_ephemeral_signer_pda(
    transaction: &Address,
    ephemeral_signer_index: u8,
) -> (Address, u8) {
    let index_bytes = ephemeral_signer_index.to_le_bytes();
    Address::find_program_address(
        &[
            SEED_PREFIX,
            transaction.as_ref(),
            SEED_EPHEMERAL_SIGNER,
            &index_bytes,
        ],
        &SQUADS_SMART_ACCOUNT_PROGRAM_ID,
    )
}

pub fn find_transaction_pda(settings: &Address, transaction_index: u64) -> (Address, u8) {
    let index_bytes = transaction_index.to_le_bytes();
    Address::find_program_address(
        &[
            SEED_PREFIX,
            settings.as_ref(),
            SEED_TRANSACTION,
            &index_bytes,
        ],
        &SQUADS_SMART_ACCOUNT_PROGRAM_ID,
    )
}

pub fn find_proposal_pda(settings: &Address, transaction_index: u64) -> (Address, u8) {
    let index_bytes = transaction_index.to_le_bytes();
    Address::find_program_address(
        &[
            SEED_PREFIX,
            settings.as_ref(),
            SEED_TRANSACTION,
            &index_bytes,
            SEED_PROPOSAL,
        ],
        &SQUADS_SMART_ACCOUNT_PROGRAM_ID,
    )
}

pub fn find_batch_transaction_pda(
    settings: &Address,
    batch_index: u64,
    transaction_index: u32,
) -> (Address, u8) {
    let batch_bytes = batch_index.to_le_bytes();
    let tx_bytes = transaction_index.to_le_bytes();
    Address::find_program_address(
        &[
            SEED_PREFIX,
            settings.as_ref(),
            SEED_TRANSACTION,
            &batch_bytes,
            SEED_BATCH_TRANSACTION,
            &tx_bytes,
        ],
        &SQUADS_SMART_ACCOUNT_PROGRAM_ID,
    )
}

pub fn find_spending_limit_pda(settings: &Address, seed: &Address) -> (Address, u8) {
    Address::find_program_address(
        &[
            SEED_PREFIX,
            settings.as_ref(),
            SEED_SPENDING_LIMIT,
            seed.as_ref(),
        ],
        &SQUADS_SMART_ACCOUNT_PROGRAM_ID,
    )
}

pub fn find_policy_pda(settings: &Address, policy_seed: u64) -> (Address, u8) {
    let seed_bytes = policy_seed.to_le_bytes();
    Address::find_program_address(
        &[SEED_PREFIX, SEED_POLICY, settings.as_ref(), &seed_bytes],
        &SQUADS_SMART_ACCOUNT_PROGRAM_ID,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdas_are_on_curve_off_curve_consistent() {
        // Sanity check: program_config is a static PDA with no inputs, so the
        // result is fully determined and should round-trip.
        let (pda, bump) = find_program_config_pda();
        let derived = Address::create_program_address(
            &[SEED_PREFIX, SEED_PROGRAM_CONFIG, &[bump]],
            &SQUADS_SMART_ACCOUNT_PROGRAM_ID,
        )
        .expect("create_program_address with derived bump succeeds");
        assert_eq!(pda, derived);
    }

    #[test]
    fn settings_pda_changes_with_account_index() {
        let (a, _) = find_settings_pda(0);
        let (b, _) = find_settings_pda(1);
        assert_ne!(a, b);
    }
}
