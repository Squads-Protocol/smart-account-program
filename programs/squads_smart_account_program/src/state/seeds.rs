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
// Seed is slightly different, to allow for off curve key without bump
pub const SEED_HOOK_AUTHORITY: &[u8] = b"hook_authority_seeds";

// Hook authority 2MTRji19YQupkpha1Rki8xvoMtEQUfMn9FB1m93DaHj8. Off curve
// without bump
pub const HOOK_AUTHORITY_PUBKEY: Pubkey = Pubkey::new_from_array([
    20, 25, 47, 2, 155, 124, 59, 36, 196, 168, 29, 160, 133, 182, 125, 32, 178, 251, 180, 88, 79,
    213, 209, 149, 172, 177, 71, 224, 215, 197, 110, 243,
]);

pub fn get_settings_signer_seeds(settings_seed: u128) -> Vec<Vec<u8>> {
    vec![
        SEED_PREFIX.to_vec(),
        SEED_SETTINGS.to_vec(),
        settings_seed.to_le_bytes().to_vec(),
    ]
}

pub fn get_policy_signer_seeds(settings_key: &Pubkey, policy_seed: u64) -> Vec<Vec<u8>> {
    vec![
        SEED_PREFIX.to_vec(),
        SEED_POLICY.to_vec(),
        settings_key.as_ref().to_vec(),
        policy_seed.to_le_bytes().to_vec(),
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
