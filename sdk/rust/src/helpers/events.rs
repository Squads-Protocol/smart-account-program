//! Decoder for the CPI-emitted "log event" pattern.
//!
//! The smart-account-program doesn't use `emit!` (which writes base64 log
//! lines). It instead emits events by self-CPIing the `LogEvent` instruction
//! with the event payload as instruction data. To observe an event, a client
//! walks the transaction response's *inner instructions*, filters to those
//! whose `program_id == SQUADS_SMART_ACCOUNT_PROGRAM_ID`, matches the first 8
//! bytes against [`LOG_EVENT_DISCRIMINATOR`], borsh-decodes the rest as
//! [`LogEventArgsV2`], and then borsh-decodes the inner `event: Vec<u8>` as
//! [`SmartAccountEvent`].
//!
//! [`parse_squads_event`] does steps 3–5 in one call.
//!
//! # Coverage
//!
//! Variants 0–7 (the non-policy events) are fully typed. Variants 8–12 of the
//! program's `SmartAccountEvent` enum (`TransactionEvent`, `ProposalEvent`,
//! `SynchronousTransactionEventV2`, `SettingsChangePolicyEvent`,
//! `PolicyEvent`) need mirror payload types for `Transaction`, `Proposal`,
//! `Policy`, `PolicyPayload`, and `LimitedSettingsAction` which are not yet
//! ported. If an emitted event uses one of those variant indices,
//! [`parse_squads_event`] returns [`ParseError::UnsupportedVariant`] — better
//! than silently returning garbage. See the module's TODO at the bottom.

use borsh::{BorshDeserialize, BorshSerialize};
use solana_address::Address;

use crate::generated::instructions::log_event::LOG_EVENT_DISCRIMINATOR;
use crate::generated::types::{
    Period, SettingsAction, SmartAccountCompiledInstruction, SmartAccountSigner,
};

/// Wrapper used by the program when emitting via self-CPI. Matches the
/// program's internal `LogEventArgsV2` (see
/// `programs/squads_smart_account_program/src/events/mod.rs::log`).
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct LogEventArgsV2 {
    pub event: Vec<u8>,
}

/// Event payload variants emitted by the smart-account-program.
///
/// Variant indices match the program's `SmartAccountEvent` enum exactly so
/// borsh decoding round-trips. Variants 8–12 are stubbed (see the module
/// docs); decoding one of those will return [`ParseError::UnsupportedVariant`].
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub enum SmartAccountEvent {
    CreateSmartAccount(CreateSmartAccountEvent),         // 0
    SynchronousTransaction(SynchronousTransactionEvent), // 1
    SynchronousSettingsTransaction(SynchronousSettingsTransactionEvent), // 2
    AddSpendingLimit(AddSpendingLimitEvent),             // 3
    RemoveSpendingLimit(RemoveSpendingLimitEvent),       // 4
    UseSpendingLimit(UseSpendingLimitEvent),             // 5
    AuthoritySettings(AuthoritySettingsEvent),           // 6
    AuthorityChange(AuthorityChangeEvent),               // 7
                                                         // Variants 8–12 (TransactionEvent, ProposalEvent,
                                                         // SynchronousTransactionEventV2, SettingsChangePolicyEvent,
                                                         // PolicyEvent) are not yet ported. See module docs.
}

// ---------------------------------------------------------------------------
// Payload mirror types — borsh-compatible with the on-wire format the program
// emits. `Settings`/`SpendingLimit` here are the codama account structs
// **without** the leading `discriminator: [u8; 8]` field (the program's
// `Settings` struct uses anchor's `#[account]` macro, which prepends the
// discriminator only when reading/writing account storage; raw `try_to_vec`
// inside an event payload does not include it).
// ---------------------------------------------------------------------------

/// Mirror of [`crate::generated::accounts::Settings`] without the
/// `discriminator` field. Use this in event payloads.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SettingsSnapshot {
    pub seed: u128,
    pub settings_authority: Address,
    pub threshold: u16,
    pub time_lock: u32,
    pub transaction_index: u64,
    pub stale_transaction_index: u64,
    pub archival_authority: Option<Address>,
    pub archivable_after: u64,
    pub bump: u8,
    pub signers: Vec<SmartAccountSigner>,
    pub account_utilization: u8,
    pub reserved1: u8,
    pub reserved2: u8,
}

/// Mirror of [`crate::generated::accounts::SpendingLimit`] without the
/// `discriminator` field.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SpendingLimitSnapshot {
    pub settings: Address,
    pub seed: Address,
    pub account_index: u8,
    pub mint: Address,
    pub amount: u64,
    pub period: Period,
    pub remaining_amount: u64,
    pub last_reset: i64,
    pub bump: u8,
    pub signers: Vec<Address>,
    pub destinations: Vec<Address>,
    pub expiration: i64,
}

// ---------------------------------------------------------------------------
// Event structs (variants 0–7)
// ---------------------------------------------------------------------------

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct CreateSmartAccountEvent {
    pub new_settings_pubkey: Address,
    pub new_settings_content: SettingsSnapshot,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SynchronousTransactionEvent {
    pub settings_pubkey: Address,
    pub account_index: u8,
    pub signers: Vec<Address>,
    pub instructions: Vec<SmartAccountCompiledInstruction>,
    pub instruction_accounts: Vec<Address>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct SynchronousSettingsTransactionEvent {
    pub settings_pubkey: Address,
    pub signers: Vec<Address>,
    pub settings: SettingsSnapshot,
    pub changes: Vec<SettingsAction>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct AddSpendingLimitEvent {
    pub settings_pubkey: Address,
    pub spending_limit_pubkey: Address,
    pub spending_limit: SpendingLimitSnapshot,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct RemoveSpendingLimitEvent {
    pub settings_pubkey: Address,
    pub spending_limit_pubkey: Address,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct UseSpendingLimitEvent {
    pub settings_pubkey: Address,
    pub spending_limit_pubkey: Address,
    pub smart_account: Address,
    pub smart_account_token_account: Address,
    pub destination: Address,
    pub destination_token_account: Address,
    pub signer: Address,
    pub mint: Address,
    pub mint_decimals: u8,
    pub amount: u64,
    pub spending_limit: SpendingLimitSnapshot,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct AuthoritySettingsEvent {
    pub settings: SettingsSnapshot,
    pub settings_pubkey: Address,
    pub authority: Address,
    pub change: SettingsAction,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct AuthorityChangeEvent {
    pub settings: SettingsSnapshot,
    pub settings_pubkey: Address,
    pub authority: Address,
    pub new_authority: Option<Address>,
}

// ---------------------------------------------------------------------------
// Decoder
// ---------------------------------------------------------------------------

/// Result type for [`parse_squads_event`].
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum ParseError {
    #[error("instruction data shorter than 8-byte discriminator")]
    DataTooShort,
    #[error("instruction discriminator does not match LogEvent")]
    NotLogEvent,
    #[error("borsh decode of LogEventArgsV2 failed: {0}")]
    DecodeArgs(String),
    #[error("borsh decode of inner SmartAccountEvent failed: {0}")]
    DecodeEvent(String),
    #[error("event variant index {0} not yet supported (variants 8–12 require policy types)")]
    UnsupportedVariant(u8),
}

/// Attempt to decode an inner instruction's `data` payload into a
/// [`SmartAccountEvent`]. Returns `Ok(None)` if the instruction is not a
/// LogEvent emission (lets callers walk every inner instruction without
/// branching first).
///
/// Callers are still responsible for matching `program_id ==
/// SQUADS_SMART_ACCOUNT_PROGRAM_ID` *before* calling this — that filter
/// belongs higher up where account-key resolution happens.
pub fn parse_squads_event(ix_data: &[u8]) -> Result<Option<SmartAccountEvent>, ParseError> {
    if ix_data.len() < 8 {
        return Err(ParseError::DataTooShort);
    }
    if ix_data[..8] != LOG_EVENT_DISCRIMINATOR {
        return Ok(None);
    }
    let args = LogEventArgsV2::try_from_slice(&ix_data[8..])
        .map_err(|e| ParseError::DecodeArgs(e.to_string()))?;
    if args.event.is_empty() {
        return Err(ParseError::DecodeEvent("empty event payload".into()));
    }
    // Peek the variant byte so we can give a meaningful error before borsh
    // walks off into garbage on unsupported variants.
    let variant = args.event[0];
    if variant > 7 {
        return Err(ParseError::UnsupportedVariant(variant));
    }
    let event = SmartAccountEvent::try_from_slice(&args.event)
        .map_err(|e| ParseError::DecodeEvent(e.to_string()))?;
    Ok(Some(event))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap_event(event: &SmartAccountEvent) -> Vec<u8> {
        // Simulate the program's emit path:
        //   ix_data = [discriminator] ++ borsh(LogEventArgsV2 { event: borsh(event) })
        let event_bytes = borsh::to_vec(event).unwrap();
        let args = LogEventArgsV2 { event: event_bytes };
        let mut out = Vec::new();
        out.extend_from_slice(&LOG_EVENT_DISCRIMINATOR);
        out.extend_from_slice(&borsh::to_vec(&args).unwrap());
        out
    }

    #[test]
    fn round_trip_remove_spending_limit() {
        let event = SmartAccountEvent::RemoveSpendingLimit(RemoveSpendingLimitEvent {
            settings_pubkey: Address::new_from_array([0x11; 32]),
            spending_limit_pubkey: Address::new_from_array([0x22; 32]),
        });
        let ix_data = wrap_event(&event);
        let decoded = parse_squads_event(&ix_data).unwrap().unwrap();
        assert_eq!(decoded, event);
    }

    #[test]
    fn non_logevent_returns_none() {
        let ix_data = vec![0xAB; 16];
        assert!(matches!(parse_squads_event(&ix_data), Ok(None)));
    }

    #[test]
    fn unsupported_variant_reports_index() {
        // Construct a fake LogEventArgsV2 whose event bytes start with variant tag 10.
        let mut event_bytes = vec![10u8];
        event_bytes.extend_from_slice(&[0u8; 32]); // some payload
        let args = LogEventArgsV2 { event: event_bytes };
        let mut ix_data = Vec::new();
        ix_data.extend_from_slice(&LOG_EVENT_DISCRIMINATOR);
        ix_data.extend_from_slice(&borsh::to_vec(&args).unwrap());
        assert_eq!(
            parse_squads_event(&ix_data),
            Err(ParseError::UnsupportedVariant(10))
        );
    }
}
