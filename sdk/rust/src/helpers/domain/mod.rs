//! Domain extension traits for codama-generated account types.
//!
//! Codama emits the raw struct layout, `from_bytes`, and (with feature
//! `fetch`) RPC helpers. It does not emit domain methods such as
//! `Settings::is_signer` or `Proposal::has_voted_approve`. These extension
//! traits supply them without touching the generated structs — the trait
//! definition lives here, and the impl block is in scope wherever callers
//! `use squads_smart_account_client::helpers::SettingsExt`.

pub mod batch;
pub mod program_config;
pub mod proposal;
pub mod settings;
pub mod spending_limit;

pub use batch::{BatchExt, BatchTransactionExt};
pub use program_config::ProgramConfigExt;
pub use proposal::ProposalExt;
pub use settings::{SettingsExt, MAX_TIME_LOCK};
pub use spending_limit::SpendingLimitExt;
