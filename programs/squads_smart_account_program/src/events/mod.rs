use anchor_lang::{
    prelude::borsh::*, prelude::*, solana_program::program::invoke_signed, Discriminator,
};

use crate::LogEventArgs;

pub mod account_events;
pub use account_events::*;

#[derive(BorshSerialize, BorshDeserialize)]
pub enum SmartAccountEvent {
    CreateSmartAccountEvent(CreateSmartAccountEvent),
    SynchronousTransactionEvent(SynchronousTransactionEvent),
    SynchronousSettingsTransactionEvent(SynchronousSettingsTransactionEvent),
    AddSpendingLimitEvent(AddSpendingLimitEvent),
    RemoveSpendingLimitEvent(RemoveSpendingLimitEvent),
    UseSpendingLimitEvent(UseSpendingLimitEvent),
}
pub struct LogAuthorityInfo<'info> {
    pub authority: AccountInfo<'info>,
    pub authority_seeds: Vec<Vec<u8>>,
    pub bump: u8,
    pub program: AccountInfo<'info>,
}
impl SmartAccountEvent {
    pub fn log<'info>(&self, authority_info: &LogAuthorityInfo<'info>) -> Result<()> {
        let mut signer_seeds: Vec<&[u8]> = authority_info
            .authority_seeds
            .iter()
            .map(|v| v.as_slice())
            .collect();
        let bump_slice = &[authority_info.bump];
        signer_seeds.push(bump_slice);

        let data = LogEventArgs {
            account_seeds: authority_info.authority_seeds.clone(),
            bump: authority_info.bump,
            event: self.try_to_vec()?,
        };
        let mut instruction_data =
            Vec::with_capacity(8 + 4 + authority_info.authority_seeds.len() + 4 + data.event.len());
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
