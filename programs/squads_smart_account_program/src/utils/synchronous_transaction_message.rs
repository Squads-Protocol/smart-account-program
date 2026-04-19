use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::solana_program::program::invoke_signed;

use crate::errors::*;
use crate::state::*;
use crate::LogEvent;

/// Sanitized and validated combination of transaction instructions and accounts
pub struct SynchronousTransactionMessage<'a, 'info> {
    pub instructions: &'a [SmartAccountCompiledInstruction],
    pub accounts: Vec<AccountInfo<'info>>,
}

impl<'a, 'info> SynchronousTransactionMessage<'a, 'info> {
    pub fn new_validated(
        settings_key: &Pubkey,
        smart_account_pubkey: &Pubkey,
        outer_signer_keys: &[Pubkey],
        instructions: &'a [SmartAccountCompiledInstruction],
        remaining_accounts: &[AccountInfo<'info>],
    ) -> Result<Self> {
        // Validate instruction indices first
        for instruction in instructions {
            require!(
                (instruction.program_id_index as usize) < remaining_accounts.len(),
                SmartAccountError::InvalidTransactionMessage
            );
            for account_index in &instruction.account_indexes {
                require!(
                    (*account_index as usize) < remaining_accounts.len(),
                    SmartAccountError::InvalidTransactionMessage
                );
            }
        }

        let mut accounts = Vec::with_capacity(remaining_accounts.len());

        // Process accounts and modify signer states
        for (_, account) in remaining_accounts.iter().enumerate() {
            let mut account_info = account.clone();

            // For remaining accounts:
            // - Set the smart account PDA as signer
            // - Prevent re-entrancy through the settings account
            // - Strip signer privilege from any outer authenticated signer
            //   that is duplicated into the inner CPI account set
            if account.key == smart_account_pubkey {
                account_info.is_signer = true;
            } else if account.key == settings_key {
                // This prevents dangerous re-entrancy
                account_info.is_writable = false;
            } else if account.is_signer && outer_signer_keys.iter().any(|key| key == account.key) {
                account_info.is_signer = false;
            }

            accounts.push(account_info);
        }

        Ok(Self {
            instructions,
            accounts,
        })
    }

    /// Executes all instructions in the message via CPI calls
    pub fn execute(&self, smart_account_seeds: &[&[u8]]) -> Result<()> {
        for instruction in self.instructions {
            let program_id = self.accounts[instruction.program_id_index as usize].key;

            // Build account metas for this instruction
            let account_metas = instruction
                .account_indexes
                .iter()
                .map(|&idx| {
                    let account = &self.accounts[idx as usize];
                    if account.is_writable {
                        AccountMeta::new(*account.key, account.is_signer)
                    } else {
                        AccountMeta::new_readonly(*account.key, account.is_signer)
                    }
                })
                .collect::<Vec<_>>();

            // Build and invoke the instruction
            let ix = Instruction {
                program_id: *program_id,
                accounts: account_metas,
                data: instruction.data.clone(),
            };

            // Check that we're not calling our self logging instruction
            LogEvent::check_instruction(&ix)?;

            let accounts_slice: Vec<AccountInfo> = instruction
                .account_indexes
                .iter()
                .map(|&idx| self.accounts[idx as usize].clone())
                .collect();

            invoke_signed(&ix, &accounts_slice, &[smart_account_seeds])?;
        }
        Ok(())
    }
}
