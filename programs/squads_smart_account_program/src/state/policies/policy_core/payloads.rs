use anchor_lang::prelude::*;

use crate::{
    state::policies::implementations::InternalFundTransferPayload, ProgramInteractionPayload,
    SettingsChangePayload, SpendingLimitPayload,
};

/// Unified enum for all policy execution payloads
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum PolicyPayload {
    InternalFundTransfer(InternalFundTransferPayload),
    ProgramInteraction(ProgramInteractionPayload),
    SpendingLimit(SpendingLimitPayload),
    SettingsChange(SettingsChangePayload),
}

