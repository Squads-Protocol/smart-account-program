//! Policy implementations
//! 
//! This module contains specific policy implementations that use the core policy framework.
//! Each policy type implements the PolicyExecutor trait for type-safe execution.

pub mod internal_fund_transfer;
pub mod spending_limit;
pub mod program_interaction;
pub mod settings_change;

pub use internal_fund_transfer::*;
pub use spending_limit::*;
pub use program_interaction::*;
pub use settings_change::*;