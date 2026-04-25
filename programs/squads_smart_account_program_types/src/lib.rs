#![allow(clippy::result_large_err)]

#[cfg(feature = "anchor")]
mod anchor_shims;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod interface;
pub mod state;
pub mod utils;

pub use errors::SmartAccountError;
pub use events::*;
pub use instructions::*;
pub use interface::*;
pub use state::*;
pub use utils::*;

pub use solana_program::pubkey::Pubkey;

#[cfg(not(feature = "testing"))]
pub const PROGRAM_ID: Pubkey =
    solana_program::pubkey!("SMRTzfY6DfH5ik3TKiyLFfXexV8uSG3d2UksSCYdunG");

#[cfg(feature = "testing")]
pub const PROGRAM_ID: Pubkey =
    solana_program::pubkey!("GyhGAqjokLwF9UXdQ2dR5Zwiup242j4mX4J1tSMKyAmD");
