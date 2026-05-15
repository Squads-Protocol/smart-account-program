//! Hand-written helpers layered on top of the codama-generated wire layer.
//!
//! Codama emits correct on-wire types, instruction builders, and CPI invokers
//! but doesn't (yet) emit PDA derivation helpers, the program's CPI-emitted
//! event payloads, or domain methods on account types. This module supplies
//! that missing layer.
//!
//! Everything here is preserved across `yarn generate:sdk` regeneration by
//! the snapshot logic in `codama/codama.ts::preserveConfigFiles`.

pub mod pda;
pub mod permissions;
pub mod transaction_message;

#[cfg(feature = "ephemeral-signers")]
pub mod ephemeral_signers;

#[cfg(feature = "tx-builder")]
pub mod versioned_tx;

pub mod domain;
pub mod events;

pub use domain::{
    BatchExt, BatchTransactionExt, ProgramConfigExt, ProposalExt, SettingsExt, SpendingLimitExt,
};
pub use events::{parse_squads_event, LogEventArgsV2, ParseError, SmartAccountEvent};
pub use pda::{
    find_batch_transaction_pda, find_ephemeral_signer_pda, find_policy_pda,
    find_program_config_pda, find_proposal_pda, find_settings_pda, find_smart_account_pda,
    find_spending_limit_pda, find_transaction_buffer_pda, find_transaction_pda,
    SEED_BATCH_TRANSACTION, SEED_EPHEMERAL_SIGNER, SEED_POLICY, SEED_PREFIX, SEED_PROGRAM_CONFIG,
    SEED_PROPOSAL, SEED_SETTINGS, SEED_SMART_ACCOUNT, SEED_SPENDING_LIMIT, SEED_TRANSACTION,
    SEED_TRANSACTION_BUFFER,
};
pub use permissions::{Permission, PermissionsExt};
pub use transaction_message::{TransactionMessageBuilder, TransactionMessageError};

#[cfg(feature = "ephemeral-signers")]
pub use ephemeral_signers::derive_ephemeral_signers;

#[cfg(feature = "tx-builder")]
pub use versioned_tx::{
    build_message_v0, build_unsigned_versioned_transaction, build_versioned_message,
    compile_unsigned_versioned_transaction, TxBuilderError,
};
