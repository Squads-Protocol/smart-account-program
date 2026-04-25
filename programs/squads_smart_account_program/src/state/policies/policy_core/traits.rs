//! Policy trait definitions.
//!
//! `PolicySizeTrait`, `PolicyPayloadConversionTrait`, `PolicyExecutionContext`
//! come from the types crate (pure data traits).
//!
//! `PolicyTrait` itself remains defined locally because it references
//! `anchor_lang::prelude::{AccountInfo, Result}` which pull in anchor — so it
//! can't live in the pure-types crate.

use anchor_lang::prelude::*;

pub use squads_smart_account_program_types::state::policies::policy_core::{
    PolicyExecutionContext, PolicyPayloadConversionTrait, PolicySizeTrait,
};

/// Core trait for policy execution — implemented by specific policy types in
/// this crate. `type CreationPayload` carries `PolicyPayloadConversionTrait`
/// from the types crate, but we rebind `Self::PolicyState` through the
/// types-crate trait so program-side impls can stay anchor-flavored.
pub trait PolicyTrait {
    /// The policy state
    type PolicyState;

    /// The creation payload
    type CreationPayload: PolicySizeTrait;

    /// The payload type used when executing this policy
    type UsagePayload;

    /// Additional arguments needed for policy execution
    type ExecutionArgs;

    /// Validate the policy state
    fn invariant(&self) -> Result<()>;

    /// Validate the payload against policy constraints before execution
    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &Self::UsagePayload,
    ) -> Result<()>;

    /// Execute the policy action with the validated payload
    fn execute_payload<'info>(
        &mut self,
        args: Self::ExecutionArgs,
        payload: &Self::UsagePayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()>;
}
