use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::{hash, Hasher};

/// Create message for vote signing
pub fn create_vote_message(proposal_key: &Pubkey, vote: u8, transaction_index: u64) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"proposal_vote_v2");
    hasher.hash(proposal_key.as_ref());
    hasher.hash(&[vote]);
    hasher.hash(&transaction_index.to_le_bytes());

    hasher
}

/// Create message for proposal creation signing
///
/// Format: hash("proposal_create_v2" || consensus_account_key || transaction_index || draft)
pub fn create_proposal_create_message(
    consensus_account_key: &Pubkey,
    transaction_index: u64,
    draft: bool,
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"proposal_create_v2");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());
    hasher.hash(&[draft as u8]);

    hasher
}

/// Create message for proposal activation signing
///
/// Format: hash("proposal_activate_v2" || proposal_key || transaction_index)
pub fn create_proposal_activate_message(
    proposal_key: &Pubkey,
    transaction_index: u64,
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"proposal_activate_v2");
    hasher.hash(proposal_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    hasher
}

/// Create message for async transaction execution signing
///
/// Format: hash("transaction_execute_v2" || transaction_key || transaction_index)
pub fn create_execute_transaction_message(
    transaction_key: &Pubkey,
    transaction_index: u64,
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"transaction_execute_v2");
    hasher.hash(transaction_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    hasher
}

/// Create message for async settings transaction execution signing
///
/// Format: hash("settings_tx_execute_v2" || transaction_key || transaction_index)
pub fn create_execute_settings_transaction_message(
    transaction_key: &Pubkey,
    transaction_index: u64,
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"settings_tx_execute_v2");
    hasher.hash(transaction_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    hasher
}

pub fn create_increment_account_index_message(
    settings: &Pubkey,
    signer_key: Pubkey,
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"increment_account_index_v2");
    hasher.hash(settings.as_ref());
    hasher.hash(signer_key.as_ref());

    hasher
}

/// Create message for batch creation signing
///
/// Format: hash("batch_create_v2" || settings_key || creator_key || account_index)
pub fn create_batch_create_message(
    settings_key: &Pubkey,
    creator_key: Pubkey,
    account_index: u8,
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"batch_create_v2");
    hasher.hash(settings_key.as_ref());
    hasher.hash(creator_key.as_ref());
    hasher.hash(&[account_index]);

    hasher
}

/// Create message for adding a transaction to a batch.
///
/// The payload_hash binds the external signer to the exact transaction content
/// being added to the batch.
///
/// Format: hash("batch_add_tx_v2" || batch_key || signer_key || transaction_index || payload_hash)
pub fn create_batch_add_transaction_message(
    batch_key: &Pubkey,
    signer_key: Pubkey,
    transaction_index: u64,
    payload_hash: &[u8; 32],
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"batch_add_tx_v2");
    hasher.hash(batch_key.as_ref());
    hasher.hash(signer_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());
    hasher.hash(payload_hash);

    hasher
}

/// Create message for executing a transaction from a batch
///
/// Format: hash("batch_execute_tx_v2" || batch_key || signer_key || transaction_index)
pub fn create_batch_execute_transaction_message(
    batch_key: &Pubkey,
    signer_key: Pubkey,
    transaction_index: u64,
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"batch_execute_tx_v2");
    hasher.hash(batch_key.as_ref());
    hasher.hash(signer_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    hasher
}

/// Create message for transaction buffer creation signing
///
/// Format: hash("tx_buffer_create_v2" || consensus_account_key || creator_key || buffer_index || account_index || final_hash || final_size)
pub fn create_transaction_buffer_create_message(
    consensus_account_key: &Pubkey,
    creator_key: Pubkey,
    buffer_index: u8,
    account_index: u8,
    final_buffer_hash: &[u8; 32],
    final_buffer_size: u16,
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"tx_buffer_create_v2");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(creator_key.as_ref());
    hasher.hash(&[buffer_index]);
    hasher.hash(&[account_index]);
    hasher.hash(final_buffer_hash);
    hasher.hash(&final_buffer_size.to_le_bytes());

    hasher
}

/// Create message for transaction buffer extension signing
///
/// Format: hash("tx_buffer_extend_v2" || buffer_key || chunk_hash)
pub fn create_transaction_buffer_extend_message(
    buffer_key: &Pubkey,
    buffer_chunk: &[u8],
) -> Hasher {
    let chunk_hash = hash(buffer_chunk);
    let mut hasher = Hasher::default();
    hasher.hash(b"tx_buffer_extend_v2");
    hasher.hash(buffer_key.as_ref());
    hasher.hash(chunk_hash.as_ref());

    hasher
}

/// Create message for transaction creation from buffer signing
///
/// Format: hash("tx_from_buffer_v2" || buffer_key || transaction_index)
pub fn create_transaction_from_buffer_message(
    buffer_key: &Pubkey,
    transaction_index: u64,
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"tx_from_buffer_v2");
    hasher.hash(buffer_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());

    hasher
}

/// Create message for synchronous transaction execution (v2 layout).
///
/// External signers commit to the exact execution context: which vault signs,
/// which accounts are passed, and what instructions run. This prevents a relayer
/// from swapping remaining_accounts or account_index after collecting signatures.
///
/// Format: hash("sync_transaction_v2" || consensus_key || tx_index || account_index
///              || num_accounts || for each: (key || is_writable) || payload_hash)
pub fn create_sync_transaction_message(
    consensus_account_key: &Pubkey,
    transaction_index: u64,
    account_index: u8,
    remaining_accounts: &[AccountInfo],
    payload_hash: &[u8; 32],
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"sync_transaction_v2");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());
    hasher.hash(&[account_index]);
    hasher.hash(&[remaining_accounts.len() as u8]);
    for acc in remaining_accounts {
        hasher.hash(acc.key.as_ref());
        hasher.hash(&[acc.is_writable as u8]);
    }
    hasher.hash(payload_hash);

    hasher
}

/// Create message for synchronous transaction execution (legacy layout).
///
/// Same security properties as the v2 variant but uses the legacy discriminator.
///
/// Format: hash("sync_transaction_legacy" || consensus_key || tx_index || account_index
///              || num_accounts || for each: (key || is_writable) || payload_hash)
pub fn create_sync_transaction_legacy_message(
    consensus_account_key: &Pubkey,
    transaction_index: u64,
    account_index: u8,
    remaining_accounts: &[AccountInfo],
    payload_hash: &[u8; 32],
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"sync_transaction_legacy");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());
    hasher.hash(&[account_index]);
    hasher.hash(&[remaining_accounts.len() as u8]);
    for acc in remaining_accounts {
        hasher.hash(acc.key.as_ref());
        hasher.hash(&[acc.is_writable as u8]);
    }
    hasher.hash(payload_hash);

    hasher
}

/// Create message for synchronous settings transaction.
///
/// Settings sync doesn't execute CPI instructions against a vault, so there's
/// no account_index or remaining_accounts to commit to. The payload_hash covers
/// the exact settings actions being applied.
///
/// Format: hash("sync_settings_tx_v2" || consensus_key || tx_index || payload_hash)
pub fn create_sync_settings_message(
    consensus_account_key: &Pubkey,
    transaction_index: u64,
    payload_hash: &[u8; 32],
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"sync_settings_tx_v2");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());
    hasher.hash(payload_hash);

    hasher
}

/// Create message for async transaction creation signing.
///
/// The payload_hash binds the external signer to the exact transaction content,
/// preventing a relayer from swapping the payload after collecting the signature.
///
/// Format: hash("transaction_create_v2" || consensus_account_key || transaction_index || payload_hash)
pub fn create_transaction_message(
    consensus_account_key: &Pubkey,
    transaction_index: u64,
    payload_hash: &[u8; 32],
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"transaction_create_v2");
    hasher.hash(consensus_account_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());
    hasher.hash(payload_hash);

    hasher
}

/// Create message for settings transaction creation signing.
///
/// The payload_hash binds the external signer to the exact settings actions,
/// preventing a relayer from swapping the actions after collecting the signature.
///
/// Format: hash("settings_tx_create_v2" || settings_key || transaction_index || payload_hash)
pub fn create_settings_transaction_create_message(
    settings_key: &Pubkey,
    transaction_index: u64,
    payload_hash: &[u8; 32],
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"settings_tx_create_v2");
    hasher.hash(settings_key.as_ref());
    hasher.hash(&transaction_index.to_le_bytes());
    hasher.hash(payload_hash);

    hasher
}

/// Create message for session key creation signing
///
/// Format: hash("create_session_key_v2" || settings_key || signer_key || session_key || expiration)
pub fn create_session_key_message(
    settings_key: &Pubkey,
    signer_key: &Pubkey,
    session_key: &Pubkey,
    session_key_expiration: u64,
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"create_session_key_v2");
    hasher.hash(settings_key.as_ref());
    hasher.hash(signer_key.as_ref());
    hasher.hash(session_key.as_ref());
    hasher.hash(&session_key_expiration.to_le_bytes());

    hasher
}

/// Create message for session key revocation signing
///
/// Format: hash("revoke_session_key_v2" || settings_key || signer_key)
pub fn create_revoke_session_key_message(
    settings_key: &Pubkey,
    signer_key: &Pubkey,
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"revoke_session_key_v2");
    hasher.hash(settings_key.as_ref());
    hasher.hash(signer_key.as_ref());

    hasher
}

/// Create message for transaction buffer close signing
///
/// Format: hash("tx_buffer_close_v2" || buffer_key || consensus_account_key)
pub fn create_transaction_buffer_close_message(
    buffer_key: &Pubkey,
    consensus_account_key: &Pubkey,
) -> Hasher {
    let mut hasher = Hasher::default();
    hasher.hash(b"tx_buffer_close_v2");
    hasher.hash(buffer_key.as_ref());
    hasher.hash(consensus_account_key.as_ref());

    hasher
}
