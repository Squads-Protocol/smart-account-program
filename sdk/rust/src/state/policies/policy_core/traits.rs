use crate::errors::SmartAccountError;

/// Trait for policy creation payloads that can be converted to policy state.
pub trait PolicyPayloadConversionTrait {
    type PolicyState;

    /// Convert the creation payload to the actual policy state.
    fn to_policy_state(self) -> Result<Self::PolicyState, SmartAccountError>;
}

/// Trait for calculating Borsh serialization sizes of policy-related structs.
pub trait PolicySizeTrait {
    /// Calculate the size when this payload is Borsh serialized.
    fn creation_payload_size(&self) -> usize;

    /// Calculate the size of the resulting policy state when Borsh serialized.
    fn policy_state_size(&self) -> usize;
}

/// The context in which the policy is being executed.
pub enum PolicyExecutionContext {
    /// The policy is being executed synchronously.
    Synchronous,
    /// The policy is being executed asynchronously.
    Asynchronous,
}
