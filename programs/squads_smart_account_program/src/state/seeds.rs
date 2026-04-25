//! Seed constants and PDA seed derivation helpers. Defined in the types
//! crate; re-exported here so existing `use crate::{SEED_*, get_*_seeds}`
//! imports keep working.

pub use squads_smart_account_program_types::state::seeds::{
    get_policy_signer_seeds, get_settings_signer_seeds, get_smart_account_seeds,
    HOOK_AUTHORITY_PUBKEY, SEED_BATCH_TRANSACTION, SEED_EPHEMERAL_SIGNER, SEED_HOOK_AUTHORITY,
    SEED_POLICY, SEED_PREFIX, SEED_PROGRAM_CONFIG, SEED_PROPOSAL, SEED_SETTINGS,
    SEED_SMART_ACCOUNT, SEED_SPENDING_LIMIT, SEED_TRANSACTION, SEED_TRANSACTION_BUFFER,
};
