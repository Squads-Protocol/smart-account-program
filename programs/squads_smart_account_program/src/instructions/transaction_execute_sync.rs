use anchor_lang::prelude::*;

use crate::{
    errors::*,
    state::*,
    utils::{validate_synchronous_consensus, SynchronousTransactionMessage},
    SmallVec,
};

use super::CompiledInstruction;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SyncTransactionArgs {
    /// The index of the smart account this transaction is for
    pub account_index: u8,
    /// The number of signers to reach threshold and adequate permissions
    pub num_signers: u8,
    /// Expected to be serialized as a SmallVec<u8, CompiledInstruction>
    pub instructions: Vec<u8>,
}

#[derive(Accounts)]
pub struct SyncTransaction<'info> {
    #[account(
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Box<Account<'info, Settings>>,
    // `remaining_accounts` must include the following accounts in the exact order:
    // 1. The exact amount of signers required to reach the threshold
    // 2. Any remaining accounts associated with the instructions
}

impl SyncTransaction<'_> {
    #[access_control(validate_synchronous_consensus( &ctx.accounts.settings, args.num_signers, &ctx.remaining_accounts))]
    pub fn sync_transaction(ctx: Context<Self>, args: SyncTransactionArgs) -> Result<()> {
        // Readonly Accounts
        let settings = &ctx.accounts.settings;

        let settings_key = settings.key();
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
            &settings.key(),
            &settings,
            &smart_account_pubkey,
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

        Ok(())
    }
}
