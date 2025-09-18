use crate::{
    instruction::LogEvent as LogEventInstruction, state::ProgramConfig,
    Policy, Proposal, Settings, SettingsTransaction, Transaction,
};
use anchor_lang::{prelude::*, Discriminator};
use solana_program::instruction::Instruction;

use crate::errors::SmartAccountError;

// ===== Log Event Instruction =====
// This custom log instruction saves bytes by not requiring a custom event authority.
// Instead it relies on the log authority to be some signing account with non-zero data in
// it owned by our program.
//
// This means even if you assign a random keypair to the smart account via
// `assign` and `allocate`, it won't be able to log events as it's data will be
// either empty, or zero-initialized to its data length
// ================================

// Allowed discriminators for logging events
const ALLOWED_DISCRIMINATORS: [[u8; 8]; 6] = [
    Settings::DISCRIMINATOR,
    Policy::DISCRIMINATOR,
    Proposal::DISCRIMINATOR,
    Transaction::DISCRIMINATOR,
    SettingsTransaction::DISCRIMINATOR,
    ProgramConfig::DISCRIMINATOR,
];

// Legacy log event instruction args.
#[allow(dead_code)]
#[derive(AnchorSerialize, AnchorDeserialize, Debug)]
pub struct LogEventArgs {
    pub account_seeds: Vec<Vec<u8>>,
    pub bump: u8,
    pub event: Vec<u8>,
}
#[derive(AnchorSerialize, AnchorDeserialize, Debug)]
pub struct LogEventArgsV2 {
    pub event: Vec<u8>,
}
#[derive(Accounts)]
#[instruction(args: LogEventArgsV2)]
pub struct LogEvent<'info> {
    #[account(
        // Any Account owned by our program can log, as long as it's discriminator is allowed
        constraint = Self::validate_log_authority(&log_authority).is_ok() @ SmartAccountError::ProtectedInstruction,
        owner = crate::id(),
    )]
    pub log_authority: Signer<'info>,
}
impl<'info> LogEvent<'info> {
    pub fn log_event(
        _ctx: Context<'_, '_, 'info, 'info, Self>,
        _args: LogEventArgsV2,
    ) -> Result<()> {
        Ok(())
    }
}

impl<'info> LogEvent<'info> {
    // Validates that a given log authority is an account that has actual data
    // in it. I.e has to have been mutated by the smart account program.
    pub fn validate_log_authority(log_authority: &Signer<'info>) -> Result<()> {
        // Get the data of the log authority
        let data = log_authority.try_borrow_data()?;

        // Extract the discriminator
        let discriminator: &[u8; 8] = data
            .get(0..8)
            .and_then(|slice| slice.try_into().ok())
            .ok_or(SmartAccountError::ProtectedInstruction)?;

        // Check if the discriminator is allowed
        require!(
            ALLOWED_DISCRIMINATORS.contains(discriminator),
            SmartAccountError::ProtectedInstruction
        );

        Ok(())
    }
    // Util fn to help check we're not invoking the log event instruction
    // from our own program during arbitrary instruction execution.
    pub fn check_instruction(ix: &Instruction) -> Result<()> {
        // Make sure we're not calling self logging instruction
        if ix.program_id == crate::ID {
            if let Some(discriminator) = ix.data.get(0..8) {
                // Check if the discriminator is the log event discriminator
                require!(
                    discriminator != LogEventInstruction::DISCRIMINATOR,
                    SmartAccountError::ProtectedInstruction
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_instruction() {
        // Invalid instruction
        let ix = Instruction {
            program_id: crate::ID,
            accounts: vec![],
            data: vec![0x05, 0x09, 0x5a, 0x8d, 0xdf, 0x86, 0x39, 0xd9],
        };
        assert!(LogEvent::check_instruction(&ix).is_err());

        // Valid instruction
        let ix = Instruction {
            // Valid since we're not calling our program
            program_id: Pubkey::default(),
            accounts: vec![],
            data: vec![0x05, 0x09, 0x5a, 0x8d, 0xdf, 0x86, 0x39, 0xd9],
        };
        assert!(LogEvent::check_instruction(&ix).is_ok());

        // Valid instruction since we're not calling the log event instruction
        let ix = Instruction {
            program_id: crate::ID,
            accounts: vec![],
            data: vec![0x05, 0x09, 0x5a, 0x8d, 0xdf, 0x86, 0x39, 0x39],
        };
        assert!(LogEvent::check_instruction(&ix).is_ok());
    }
}
