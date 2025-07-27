//! Policy system for smart accounts
//!
//! This module provides a unified policy framework for smart accounts, allowing
//! different types of policies to be executed through a common consensus mechanism.
//!
//! ## Structure
//!
//! - `core/`: Core policy framework (Policy struct, traits, payloads)
//! - `implementations/`: Specific policy implementations
//! - `tests/`: Test modules for framework and implementations

pub mod implementations;
pub mod policy_core;
pub mod tests;
mod utils;

pub use policy_core::*;

// Re-export implementations for convenience
pub use implementations::*;
