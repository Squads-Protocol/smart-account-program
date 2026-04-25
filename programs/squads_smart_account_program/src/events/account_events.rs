//! Event struct definitions — re-exported from the types crate.
pub use squads_smart_account_program_types::{
    AddSpendingLimitEvent, AuthorityChangeEvent, AuthoritySettingsEvent, CreateSmartAccountEvent,
    PolicyEvent, PolicyEventType, ProposalEvent, ProposalEventType, RemoveSpendingLimitEvent,
    SettingsChangePolicyEvent, SynchronousSettingsTransactionEvent, SynchronousTransactionEvent,
    SynchronousTransactionEventPayload, SynchronousTransactionEventV2, TransactionContent,
    TransactionEvent, TransactionEventType, UseSpendingLimitEvent,
};
