//! SmallVec — re-exported from the types crate. Anchor's `AnchorSerialize` /
//! `AnchorDeserialize` are re-exports of borsh's traits, so the types crate's
//! borsh impls satisfy anchor's type constraints.

pub use squads_smart_account_program_types::SmallVec;
