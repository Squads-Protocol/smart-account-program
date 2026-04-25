//! Event data lives in the types crate; this module re-exports it and adds a
//! program-side extension trait `SmartAccountEventExt` that provides the
//! `log()` CPI needed to emit events on-chain.

use anchor_lang::{
    prelude::borsh::*, prelude::*, solana_program::program::invoke_signed, Discriminator,
};

pub use squads_smart_account_program_types::SmartAccountEvent;

use crate::LogEventArgsV2;

pub mod account_events;
pub use account_events::*;

/// Wraps the signer authority used to emit a CPI-style event.
pub struct LogAuthorityInfo<'info> {
    pub authority: AccountInfo<'info>,
    pub authority_seeds: Vec<Vec<u8>>,
    pub bump: u8,
    pub program: AccountInfo<'info>,
}

/// Extension trait that lets `SmartAccountEvent` self-publish via an
/// `invoke_signed` call to `log_event` on this program.
pub trait SmartAccountEventExt {
    fn log(&self, authority_info: &LogAuthorityInfo) -> Result<()>;
}

impl SmartAccountEventExt for SmartAccountEvent {
    fn log(&self, authority_info: &LogAuthorityInfo) -> Result<()> {
        let mut signer_seeds: Vec<&[u8]> = authority_info
            .authority_seeds
            .iter()
            .map(|v| v.as_slice())
            .collect();
        let bump_slice = &[authority_info.bump];
        signer_seeds.push(bump_slice);

        let data = LogEventArgsV2 {
            event: self.try_to_vec()?,
        };
        let mut instruction_data = Vec::with_capacity(8 + 4 + data.event.len());
        instruction_data.extend_from_slice(&crate::instruction::LogEvent::DISCRIMINATOR);
        instruction_data.extend_from_slice(&data.try_to_vec()?);

        let ix = solana_program::instruction::Instruction {
            program_id: authority_info.program.key(),
            accounts: vec![AccountMeta::new_readonly(
                authority_info.authority.key(),
                true,
            )],
            data: instruction_data,
        };
        let mut authority_account_info = authority_info.authority.clone();
        authority_account_info.is_signer = true;

        invoke_signed(&ix, &[authority_account_info], &[signer_seeds.as_slice()])?;
        Ok(())
    }
}
