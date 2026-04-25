//! Policy implementations
//!
//! This module contains specific policy implementations that use the core policy framework.
//! Each policy type implements the PolicyExecutor trait for type-safe execution.

pub mod internal_fund_transfer;
pub mod program_interaction;
pub mod settings_change;
pub mod spending_limit_policy;

pub use internal_fund_transfer::*;
pub use program_interaction::*;
pub use settings_change::*;
pub use spending_limit_policy::*;
