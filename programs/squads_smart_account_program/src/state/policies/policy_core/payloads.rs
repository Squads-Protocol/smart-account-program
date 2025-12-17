use anchor_lang::prelude::*;

use crate::{
    state::policies::implementations::InternalFundTransferPayload,
    InternalFundTransferPolicyCreationPayload, ProgramInteractionPayload,
    ProgramInteractionPolicyCreationPayload, ProgramInteractionPolicyCreationPayloadLegacy,
    SettingsChangePayload, SettingsChangePolicyCreationPayload, SpendingLimitPayload,
    SpendingLimitPolicyCreationPayload,
};

use super::PolicySizeTrait;

/// Unified enum for all policy creation payloads
/// These are used in SettingsAction::PolicyCreate to specify which type of policy to create
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum PolicyCreationPayload {
    InternalFundTransfer(InternalFundTransferPolicyCreationPayload),
    SpendingLimit(SpendingLimitPolicyCreationPayload),
    SettingsChange(SettingsChangePolicyCreationPayload),
    LegacyProgramInteraction(ProgramInteractionPolicyCreationPayloadLegacy),
    ProgramInteraction(ProgramInteractionPolicyCreationPayload),
}

impl PolicyCreationPayload {
    /// Calculate the size of the resulting policy data after creation
    pub fn policy_state_size(&self) -> usize {
        // 1 for the Wrapper enum type
        1 + match self {
            PolicyCreationPayload::InternalFundTransfer(payload) => payload.policy_state_size(),
            PolicyCreationPayload::SpendingLimit(payload) => payload.policy_state_size(),
            PolicyCreationPayload::SettingsChange(payload) => payload.policy_state_size(),
            PolicyCreationPayload::LegacyProgramInteraction(payload) => payload.policy_state_size(),
            PolicyCreationPayload::ProgramInteraction(payload) => payload.policy_state_size(),
        }
    }
}

/// Unified enum for all policy execution payloads
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum PolicyPayload {
    InternalFundTransfer(InternalFundTransferPayload),
    ProgramInteraction(ProgramInteractionPayload),
    SpendingLimit(SpendingLimitPayload),
    SettingsChange(SettingsChangePayload),
}
