use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::solana_program::program::invoke_signed;

use crate::errors::*;
use crate::state::*;

/// Sanitized and validated combination of transaction instructions and accounts
pub struct SynchronousTransactionMessage<'info> {
    pub instructions: Vec<SmartAccountCompiledInstruction>,
    pub accounts: Vec<AccountInfo<'info>>,
}

impl<'info> SynchronousTransactionMessage<'info> {
    pub fn new_validated(
        settings_key: &Pubkey,
        smart_account_pubkey: &Pubkey,
        consensus_account_signers: &[SmartAccountSigner],
        instructions: Vec<SmartAccountCompiledInstruction>,
        remaining_accounts: &[AccountInfo<'info>],
    ) -> Result<Self> {
        // Validate instruction indices first
        for instruction in &instructions {
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
        for (i, account) in remaining_accounts.iter().enumerate() {
            let mut account_info = account.clone();

            // For remaining accounts:
            // - Set account as signer
            // - Remove signer privilege from any smart account signers
            // - Set smart account as non-writable
            if account.key == smart_account_pubkey {
                account_info.is_signer = true;
            } else if account.key == settings_key {
                // This prevents dangerous re-entrancy
                account_info.is_writable = false;
            } else if consensus_account_signers.iter().any(|signer| &signer.key == account.key) && account.is_signer {
                // We may want to remove this so that a signer can be a rent
                // or feepayer on any of the CPI instructions
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
        for instruction in &self.instructions {
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
