use anchor_lang::prelude::Pubkey;

pub const SEED_PREFIX: &[u8] = b"smart_account";
pub const SEED_PROGRAM_CONFIG: &[u8] = b"program_config";
pub const SEED_SETTINGS: &[u8] = b"settings";
pub const SEED_PROPOSAL: &[u8] = b"proposal";
pub const SEED_TRANSACTION: &[u8] = b"transaction";
pub const SEED_BATCH_TRANSACTION: &[u8] = b"batch_transaction";
pub const SEED_SMART_ACCOUNT: &[u8] = b"smart_account";
pub const SEED_EPHEMERAL_SIGNER: &[u8] = b"ephemeral_signer";
pub const SEED_SPENDING_LIMIT: &[u8] = b"spending_limit";
pub const SEED_TRANSACTION_BUFFER: &[u8] = b"transaction_buffer";
pub const SEED_POLICY: &[u8] = b"policy";

pub fn get_settings_signer_seeds(settings_seed: u128) -> Vec<Vec<u8>> {
    vec![
        SEED_PREFIX.to_vec(),
        SEED_SETTINGS.to_vec(),
        settings_seed.to_le_bytes().to_vec(),
    ]
}

/// Derives the account seeds for a given smart account based on the settings key and account index
pub fn get_smart_account_seeds<'a>(
    settings_key: &'a Pubkey,
    account_index_bytes: &'a [u8],
) -> [&'a [u8]; 4] {
    [
        SEED_PREFIX,
        settings_key.as_ref(),
        SEED_SMART_ACCOUNT,
        account_index_bytes,
    ]
}
