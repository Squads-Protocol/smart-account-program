//! Versioned-message and unsigned-transaction builders.
//!
//! Gated on feature `tx-builder` because it pulls in the granular
//! `solana-message`, `solana-hash`, and `solana-transaction` crates (NOT the
//! umbrella `solana-sdk`). Only enable this feature if you actually need to
//! compose full transactions client-side; one-shot instruction construction
//! works fine without it.
//!
//! # Why this exists
//!
//! Codama emits instruction builders but stops at
//! `solana_instruction::Instruction`. To submit those instructions you need
//! to wrap them into a `VersionedMessage` (with a payer, recent blockhash,
//! and optional address-lookup-table accounts) and then into a
//! `VersionedTransaction`. This module supplies that final layer.

use solana_hash::Hash;
use solana_instruction::Instruction;
use solana_message::v0::Message as MessageV0;
use solana_message::AddressLookupTableAccount;
use solana_message::VersionedMessage;
use solana_transaction::versioned::VersionedTransaction;
use solana_transaction::Signature;

use solana_address::Address;

/// Compile instructions into a V0 message.
///
/// `payer` becomes account index 0 (the fee payer / primary signer).
/// `address_lookup_table_accounts` may be empty; supply them if any of the
/// instruction accounts reference keys via ALTs.
pub fn build_message_v0(
    payer: &Address,
    instructions: &[Instruction],
    address_lookup_table_accounts: &[AddressLookupTableAccount],
    recent_blockhash: Hash,
) -> Result<MessageV0, TxBuilderError> {
    MessageV0::try_compile(
        payer,
        instructions,
        address_lookup_table_accounts,
        recent_blockhash,
    )
    .map_err(|e| TxBuilderError::Compile(e.to_string()))
}

/// Compile instructions into a [`VersionedMessage::V0`].
///
/// Thin wrapper over [`build_message_v0`] for callers that want the enum form.
pub fn build_versioned_message(
    payer: &Address,
    instructions: &[Instruction],
    address_lookup_table_accounts: &[AddressLookupTableAccount],
    recent_blockhash: Hash,
) -> Result<VersionedMessage, TxBuilderError> {
    Ok(VersionedMessage::V0(build_message_v0(
        payer,
        instructions,
        address_lookup_table_accounts,
        recent_blockhash,
    )?))
}

/// Build a [`VersionedTransaction`] with placeholder (zeroed) signatures sized
/// to the message's required-signature count. Callers fill in signatures with
/// their own signing infrastructure before submission.
///
/// The signature placeholder count matches `message.header().num_required_signatures`,
/// which is the on-wire validation requirement Solana checks before accepting
/// the transaction.
pub fn build_unsigned_versioned_transaction(message: VersionedMessage) -> VersionedTransaction {
    let num_required = usize::from(message.header().num_required_signatures);
    VersionedTransaction {
        signatures: vec![Signature::default(); num_required],
        message,
    }
}

/// One-shot helper: takes everything needed and produces an unsigned versioned
/// transaction whose signatures are zeroed placeholders. Compose with the
/// codama-emitted instruction builders like so:
///
/// ```ignore
/// use squads_smart_account_client::instructions::CreateTransactionBuilder;
/// use squads_smart_account_client::helpers::{find_settings_pda, find_transaction_pda};
/// use squads_smart_account_client::helpers::versioned_tx::compile_unsigned_versioned_transaction;
///
/// let (settings, _) = find_settings_pda(0);
/// let (tx_pda, _) = find_transaction_pda(&settings, 1);
/// let ix = CreateTransactionBuilder::new()
///     .settings(settings)
///     .transaction(tx_pda)
///     // ... etc
///     .instruction();
///
/// let tx = compile_unsigned_versioned_transaction(&payer, &[ix], &[], blockhash)?;
/// // sign tx.signatures with payer's signer, then submit
/// ```
pub fn compile_unsigned_versioned_transaction(
    payer: &Address,
    instructions: &[Instruction],
    address_lookup_table_accounts: &[AddressLookupTableAccount],
    recent_blockhash: Hash,
) -> Result<VersionedTransaction, TxBuilderError> {
    let message = build_versioned_message(
        payer,
        instructions,
        address_lookup_table_accounts,
        recent_blockhash,
    )?;
    Ok(build_unsigned_versioned_transaction(message))
}

#[derive(Debug, thiserror::Error)]
pub enum TxBuilderError {
    #[error("MessageV0 compile failed: {0}")]
    Compile(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_instruction::AccountMeta;

    fn dummy_address(byte: u8) -> Address {
        Address::new_from_array([byte; 32])
    }

    #[test]
    fn build_message_v0_with_one_instruction() {
        let payer = dummy_address(0x01);
        let prog = dummy_address(0xAA);
        let ix = Instruction {
            program_id: prog,
            accounts: vec![AccountMeta::new(dummy_address(0x02), false)],
            data: vec![0xDE, 0xAD],
        };
        let blockhash = Hash::new_from_array([0u8; 32]);
        let msg = build_message_v0(&payer, &[ix], &[], blockhash).unwrap();

        // Payer must be account index 0 and the sole required signer.
        assert_eq!(msg.account_keys[0], payer);
        assert_eq!(msg.header.num_required_signatures, 1);
        assert_eq!(msg.recent_blockhash, blockhash);
        assert_eq!(msg.instructions.len(), 1);
    }

    #[test]
    fn unsigned_transaction_has_placeholder_signatures() {
        let payer = dummy_address(0x01);
        let ix = Instruction {
            program_id: dummy_address(0xAA),
            accounts: vec![],
            data: vec![],
        };
        let blockhash = Hash::new_from_array([0u8; 32]);
        let tx = compile_unsigned_versioned_transaction(&payer, &[ix], &[], blockhash).unwrap();
        assert_eq!(tx.signatures.len(), 1);
        assert_eq!(tx.signatures[0], Signature::default());
    }
}
