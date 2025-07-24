use anchor_lang::prelude::*;

/// Trait for policy creation payloads that can be converted to policy state
pub trait PolicyPayloadConversionTrait {
    type PolicyState;

    /// Convert the creation payload to the actual policy state
    fn to_policy_state(self) -> Self::PolicyState;
}

/// Trait for calculating Borsh serialization sizes of policy-related structs
pub trait PolicySizeTrait {
    /// Calculate the size when this payload is Borsh serialized
    fn creation_payload_size(&self) -> usize;

    /// Calculate the size of the resulting policy state when Borsh serialized
    fn policy_state_size(&self) -> usize;
}

/// Core trait for policy execution - implemented by specific policy types
pub trait PolicyTrait {
    /// The policy state
    type PolicyState;

    /// The creation payload
    type CreationPayload: PolicyPayloadConversionTrait<PolicyState = Self::PolicyState>
        + PolicySizeTrait;

    /// The payload type used when executing this policy
    type UsagePayload;

    /// Additional arguments needed for policy execution
    type ExecutionArgs;

    /// Validate the policy state
    fn invariant(&self) -> Result<()>;

    /// Validate the payload against policy constraints before execution
    fn validate_payload(&self, payload: &Self::UsagePayload) -> Result<()>;

    /// Execute the policy action with the validated payload
    fn execute_payload<'info>(
        &mut self,
        args: Self::ExecutionArgs,
        payload: &Self::UsagePayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()>;

}
