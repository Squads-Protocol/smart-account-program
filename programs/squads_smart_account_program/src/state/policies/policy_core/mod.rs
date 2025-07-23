//! Core policy framework
//!
//! This module contains the fundamental policy framework including:
//! - Policy struct and PolicyType enum
//! - PolicyExecutor trait for type-safe execution
//! - PolicyPayload enum for unified payloads
//! - Core consensus integration

pub mod creation_payloads;
pub mod payloads;
pub mod policy;
pub mod traits;

pub use creation_payloads::*;
pub use payloads::*;
pub use policy::*;
pub use traits::*;
