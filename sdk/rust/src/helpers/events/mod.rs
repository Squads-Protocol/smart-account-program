//! Decoder for the CPI-emitted "log event" pattern + all 13 event payload
//! shapes.
//!
//! The smart-account-program emits events by self-CPIing a `LogEvent`
//! instruction whose `data` is `[discriminator(8 bytes)] ++
//! borsh(LogEventArgsV2 { event: Vec<u8> })`. The inner `event` bytes are
//! `borsh(SmartAccountEvent)`. [`parse_squads_event`] walks all three layers
//! in one call.
//!
//! # Submodules
//! - [`snapshots`] — account-state mirror types used inside event payloads
//!   (Settings, Transaction, Proposal, etc., without the discriminator field).
//! - [`policy`] — policy data types ported from the program's
//!   `state/policies/` subtree (data-only; invariants live on-chain).
//! - [`variants`] — the 13 event payload structs.
//!
//! # Drift caveat
//!
//! Codama generates SDK types from `idl/squads_smart_account_program.json`,
//! which can drift behind the actual program source if the IDL hasn't been
//! regenerated. The snapshot mirrors here track the **current program
//! source**, not the IDL. If you find a decode mismatch, the program's
//! `state/` source is the canonical reference.

pub mod policy;
pub mod snapshots;
pub mod variants;

use borsh::{BorshDeserialize, BorshSerialize};

use crate::generated::instructions::log_event::LOG_EVENT_DISCRIMINATOR;

pub use snapshots::{
    ConsensusAccountType, Payload, PolicyActionPayloadDetails, ProposalSnapshot, SettingsSnapshot,
    SettingsTransactionSnapshot, SpendingLimitSnapshot, TransactionPayload,
    TransactionPayloadDetails, TransactionSnapshot,
};
pub use variants::{
    AddSpendingLimitEvent, AuthorityChangeEvent, AuthoritySettingsEvent, CreateSmartAccountEvent,
    PolicyEvent, PolicyEventType, ProposalEvent, ProposalEventType, RemoveSpendingLimitEvent,
    SettingsChangePolicyEvent, SynchronousSettingsTransactionEvent, SynchronousTransactionEvent,
    SynchronousTransactionEventPayload, SynchronousTransactionEventV2, TransactionContent,
    TransactionEvent, TransactionEventType, UseSpendingLimitEvent,
};

/// Inner-instruction wrapper used by the program's `LogEvent` self-CPI.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct LogEventArgsV2 {
    pub event: Vec<u8>,
}

/// Variant ordering matches the program's `SmartAccountEvent` enum exactly so
/// borsh decoding round-trips on the bytes the program emits.
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
    Transaction(TransactionEvent),                       // 8
    Proposal(ProposalEvent),                             // 9
    SynchronousTransactionV2(SynchronousTransactionEventV2), // 10
    SettingsChangePolicy(SettingsChangePolicyEvent),     // 11
    Policy(PolicyEvent),                                 // 12
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum ParseError {
    #[error("instruction data shorter than 8-byte discriminator")]
    DataTooShort,
    #[error("borsh decode of LogEventArgsV2 failed: {0}")]
    DecodeArgs(String),
    #[error("borsh decode of inner SmartAccountEvent failed: {0}")]
    DecodeEvent(String),
}

/// Decode an inner instruction's `data` as a `SmartAccountEvent`. Returns
/// `Ok(None)` if the instruction is not a LogEvent emission (the
/// discriminator doesn't match) — this lets callers walk every inner
/// instruction without branching first.
///
/// Callers are still responsible for filtering by `program_id ==
/// SQUADS_SMART_ACCOUNT_PROGRAM_ID` before calling — that lookup belongs
/// higher up where account-key resolution happens.
pub fn parse_squads_event(ix_data: &[u8]) -> Result<Option<SmartAccountEvent>, ParseError> {
    if ix_data.len() < 8 {
        return Err(ParseError::DataTooShort);
    }
    if ix_data[..8] != LOG_EVENT_DISCRIMINATOR {
        return Ok(None);
    }
    let args = LogEventArgsV2::try_from_slice(&ix_data[8..])
        .map_err(|e| ParseError::DecodeArgs(e.to_string()))?;
    let event = SmartAccountEvent::try_from_slice(&args.event)
        .map_err(|e| ParseError::DecodeEvent(e.to_string()))?;
    Ok(Some(event))
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_address::Address;

    fn wrap_event(event: &SmartAccountEvent) -> Vec<u8> {
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
        assert_eq!(parse_squads_event(&ix_data).unwrap().unwrap(), event);
    }

    #[test]
    fn round_trip_policy_event_create() {
        let event = SmartAccountEvent::Policy(PolicyEvent {
            event_type: PolicyEventType::Create,
            settings_pubkey: Address::new_from_array([0x33; 32]),
            policy_pubkey: Address::new_from_array([0x44; 32]),
            policy: None,
        });
        let ix_data = wrap_event(&event);
        assert_eq!(parse_squads_event(&ix_data).unwrap().unwrap(), event);
    }

    #[test]
    fn non_logevent_returns_none() {
        let ix_data = vec![0xAB; 16];
        assert!(matches!(parse_squads_event(&ix_data), Ok(None)));
    }
}
