use anchor_lang::{prelude::*, Discriminator};
use anchor_lang::solana_program::{
    entrypoint::ProgramResult,
    system_program,
    sysvar::instructions::{self, load_instruction_at_checked},
};

use crate::errors::SmartAccountError;

const ADVANCE_NONCE_INSTRUCTION_DISCRIMINATOR: [u8; 4] = [4, 0, 0, 0];

anchor_lang::solana_program::entrypoint!(process_instruction);

pub fn process_instruction<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    data: &[u8],
) -> ProgramResult {
    if should_bypass_nonce_validation(data) {
        return crate::entry(program_id, accounts, data);
    }

    // Every non-bypassed instruction must have the Instructions sysvar appended
    // as the last account. Strip it before forwarding to Anchor so that all
    // instruction contexts see a consistent account list.
    let (instructions_sysvar, anchor_accounts) = accounts
        .split_last()
        .ok_or(anchor_lang::solana_program::program_error::ProgramError::NotEnoughAccountKeys)?;

    validate_never_nonce(instructions_sysvar)?;

    crate::entry(program_id, anchor_accounts, data)
}

fn should_bypass_nonce_validation(data: &[u8]) -> bool {
    let Some(discriminator) = data.get(..8) else {
        return true;
    };

    discriminator == anchor_lang::idl::IDL_IX_TAG_LE.as_ref()
        || discriminator == anchor_lang::event::EVENT_IX_TAG_LE.as_ref()
        // LogEvent is invoked via self-CPI for event logging. It only emits
        // program logs and does not modify any state — safe to bypass.
        || discriminator == crate::instruction::LogEvent::DISCRIMINATOR
}

fn validate_never_nonce(instructions_sysvar: &AccountInfo) -> Result<()> {
    require_keys_eq!(
        *instructions_sysvar.key,
        instructions::ID,
        SmartAccountError::InvalidInstructionsSysvar
    );

    let instruction = load_instruction_at_checked(0, instructions_sysvar)
        .map_err(|_| error!(SmartAccountError::InvalidInstructionsSysvar))?;

    require!(
        !is_advance_nonce_instruction(&instruction),
        SmartAccountError::DurableNonceForbidden
    );

    Ok(())
}

fn is_advance_nonce_instruction(instruction: &anchor_lang::solana_program::instruction::Instruction) -> bool {
    instruction.program_id == system_program::ID
        && instruction.data.get(..4) == Some(&ADVANCE_NONCE_INSTRUCTION_DISCRIMINATOR)
}
