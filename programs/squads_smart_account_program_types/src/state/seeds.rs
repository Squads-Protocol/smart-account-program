use solana_pubkey::Pubkey;

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

#[cfg(not(feature = "testing"))]
pub const SEED_HOOK_AUTHORITY: &[u8] = b"hook_authority_seeds";

#[cfg(feature = "testing")]
pub const SEED_HOOK_AUTHORITY: &[u8] = b"hook_authority_seeds_test";

#[cfg(not(feature = "testing"))]
pub const HOOK_AUTHORITY_PUBKEY: Pubkey = Pubkey::new_from_array([
    20, 25, 47, 2, 155, 124, 59, 36, 196, 168, 29, 160, 133, 182, 125, 32, 178, 251, 180, 88, 79,
    213, 209, 149, 172, 177, 71, 224, 215, 197, 110, 243,
]);

#[cfg(feature = "testing")]
pub const HOOK_AUTHORITY_PUBKEY: Pubkey = Pubkey::new_from_array([
    32, 214, 95, 168, 10, 233, 119, 125, 45, 249, 95, 236, 95, 70, 192, 202, 150, 140, 8, 162, 126,
    245, 141, 215, 164, 36, 97, 134, 2, 197, 62, 175,
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

#[cfg(all(test, feature = "ephemeral-signers"))]
mod tests {
    use super::*;

    #[test]
    fn test_hook_authority_pubkey() {
        let address =
            Pubkey::create_program_address(&[SEED_HOOK_AUTHORITY], &crate::PROGRAM_ID).unwrap();
        assert_eq!(address, HOOK_AUTHORITY_PUBKEY);
    }

    #[cfg(feature = "testing")]
    #[test]
    fn test_testing_hook_authority_pubkey() {
        let test_program_id =
            Pubkey::from_str("GyhGAqjokLwF9UXdQ2dR5Zwiup242j4mX4J1tSMKyAmD").unwrap();
        let address =
            Pubkey::create_program_address(&[SEED_HOOK_AUTHORITY], &test_program_id).unwrap();
        assert_eq!(address, HOOK_AUTHORITY_PUBKEY);
    }
}
