use account_events::SynchronousTransactionEvent;
use anchor_lang::prelude::*;

use crate::{
    consensus::ConsensusAccount, consensus_trait::ConsensusAccountType, errors::*, events::*, program::SquadsSmartAccountProgram, state::*, utils::{validate_synchronous_consensus, SynchronousTransactionMessage}, SmallVec
};

use super::CompiledInstruction;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct LegacySyncTransactionArgs {
    /// The index of the smart account this transaction is for
    pub account_index: u8,
    /// The number of signers to reach threshold and adequate permissions
    pub num_signers: u8,
    /// Expected to be serialized as a SmallVec<u8, CompiledInstruction>
    pub instructions: Vec<u8>,
}

#[derive(Accounts)]
pub struct LegacySyncTransaction<'info> {
    #[account(
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok(),
        constraint = consensus_account.account_type() == ConsensusAccountType::Settings
    )]
    pub consensus_account: Box<InterfaceAccount<'info, ConsensusAccount>>,
    pub program: Program<'info, SquadsSmartAccountProgram>,
    // `remaining_accounts` must include the following accounts in the exact order:
    // 1. The exact amount of signers required to reach the threshold
    // 2. Any remaining accounts associated with the instructions
}

impl LegacySyncTransaction<'_> {
    #[access_control(validate_synchronous_consensus( &ctx.accounts.consensus_account, args.num_signers, &ctx.remaining_accounts))]
    pub fn sync_transaction(ctx: Context<Self>, args: LegacySyncTransactionArgs) -> Result<()> {
        // Wrapper consensus account
        let consensus_account = &ctx.accounts.consensus_account;
        // Readonly Accounts
        let settings = consensus_account.read_only_settings()?;

        let settings_key = consensus_account.key();
        // Deserialize the instructions
        let compiled_instructions =
            SmallVec::<u8, CompiledInstruction>::try_from_slice(&args.instructions)
                .map_err(|_| SmartAccountError::InvalidInstructionArgs)?;
        // Convert to SmartAccountCompiledInstruction
        let settings_compiled_instructions: Vec<SmartAccountCompiledInstruction> =
            Vec::from(compiled_instructions)
                .into_iter()
                .map(SmartAccountCompiledInstruction::from)
                .collect();

        let smart_account_seeds = &[
            SEED_PREFIX,
            settings_key.as_ref(),
            SEED_SMART_ACCOUNT,
            &args.account_index.to_le_bytes(),
        ];

        let (smart_account_pubkey, smart_account_bump) =
            Pubkey::find_program_address(smart_account_seeds, ctx.program_id);

        // Get the signer seeds for the smart account
        let smart_account_signer_seeds = &[
            smart_account_seeds[0],
            smart_account_seeds[1],
            smart_account_seeds[2],
            smart_account_seeds[3],
            &[smart_account_bump],
        ];

        let executable_message = SynchronousTransactionMessage::new_validated(
            &settings_key,
            &smart_account_pubkey,
            &settings.signers,
            settings_compiled_instructions,
            &ctx.remaining_accounts,
        )?;

        // Execute the transaction message instructions one-by-one.
        // NOTE: `execute_message()` calls `self.to_instructions_and_accounts()`
        // which in turn calls `take()` on
        // `self.message.instructions`, therefore after this point no more
        // references or usages of `self.message` should be made to avoid
        // faulty behavior.
        executable_message.execute(smart_account_signer_seeds)?;

        // Log the event
        let event = SynchronousTransactionEvent {
            settings_pubkey: settings_key,
            signers: ctx.remaining_accounts[..args.num_signers as usize]
                .iter()
                .map(|acc| acc.key.clone())
                .collect(),
            account_index: args.account_index,
            instructions: executable_message.instructions,
            instruction_accounts: executable_message
                .accounts
                .iter()
                .map(|a| a.key.clone())
                .collect(),
        };
        let log_authority_info = LogAuthorityInfo {
            authority: consensus_account.to_account_info(),
            authority_seeds: get_settings_signer_seeds(settings.seed),
            bump: settings.bump,
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::SynchronousTransactionEvent(event).log(&log_authority_info)?;
        Ok(())
    }
}
