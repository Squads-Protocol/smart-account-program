use crate::errors::*;
use crate::instructions::*;
use crate::state::*;
use anchor_lang::{prelude::*, system_program};

#[derive(Accounts)]
pub struct CreateTransactionFromBuffer<'info> {
    // The context needed for the CreateTransaction instruction
    pub transaction_create: CreateTransaction<'info>,

    #[account(
        mut,
        close = creator,
        // Only the creator can turn the buffer into a transaction and reclaim
        // the rent
        constraint = transaction_buffer.creator == creator.key() @ SmartAccountError::Unauthorized,
        seeds = [
            SEED_PREFIX,
            transaction_create.consensus_account.key().as_ref(),
            SEED_TRANSACTION_BUFFER,
            creator.key().as_ref(),
            &transaction_buffer.buffer_index.to_le_bytes(),
        ],
        bump
    )]
    pub transaction_buffer: Box<Account<'info, TransactionBuffer>>,

    // Anchor doesn't allow us to use the creator inside of
    // transaction_create, so we just re-pass it here with the same constraint
    #[account(
        mut,
        address = transaction_create.creator.key(),
    )]
    pub creator: Signer<'info>,
}

impl<'info> CreateTransactionFromBuffer<'info> {
    pub fn validate(&self, args: &CreateTransactionArgs) -> Result<()> {
        let transaction_buffer_account = &self.transaction_buffer;

        // Check that the transaction message is "empty"
        require!(
            args.transaction_message == vec![0, 0, 0, 0, 0, 0],
            SmartAccountError::InvalidInstructionArgs
        );

        // Validate that the final hash matches the buffer
        transaction_buffer_account.validate_hash()?;

        // Validate that the final size is correct
        transaction_buffer_account.validate_size()?;
        Ok(())
    }
    /// Create a new Transaction from a completed transaction buffer account.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn create_transaction_from_buffer(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: CreateTransactionArgs,
    ) -> Result<()> {
        // Account infos necessary for reallocation
        let transaction_account_info = &ctx
            .accounts
            .transaction_create
            .transaction
            .to_account_info();
        let rent_payer_account_info = &ctx
            .accounts
            .transaction_create
            .rent_payer
            .to_account_info();

        let system_program = &ctx
            .accounts
            .transaction_create
            .system_program
            .to_account_info();

        // Read-only accounts
        let transaction_buffer = &ctx.accounts.transaction_buffer;

        // Calculate the new required length of the transaction account,
        // since it was initialized with an empty transaction message
        let new_len =
            Transaction::size(args.ephemeral_signers, transaction_buffer.buffer.as_slice())?;

        // Calculate the rent exemption for new length
        let rent_exempt_lamports = Rent::get().unwrap().minimum_balance(new_len).max(1);

        // Check the difference between the rent exemption and the current lamports
        let top_up_lamports =
            rent_exempt_lamports.saturating_sub(transaction_account_info.lamports());

        // System Transfer the remaining difference to the transaction account
        let transfer_context = CpiContext::new(
            system_program.to_account_info(),
            system_program::Transfer {
                from: rent_payer_account_info.clone(),
                to: transaction_account_info.clone(),
            },
        );
        system_program::transfer(transfer_context, top_up_lamports)?;

        // Reallocate the transaction account to the new length of the
        // actual transaction message
        AccountInfo::realloc(&transaction_account_info, new_len, true)?;

        // Create the args for the `create_transaction` instruction
        let create_args = CreateTransactionArgs {
            account_index: args.account_index,
            ephemeral_signers: args.ephemeral_signers,
            transaction_message: transaction_buffer.buffer.clone(),
            memo: args.memo,
        };
        // Create the context for the `create_transaction` instruction
        let context = Context::new(
            ctx.program_id,
            &mut ctx.accounts.transaction_create,
            ctx.remaining_accounts,
            ctx.bumps.transaction_create,
        );

        // Call the `create_transaction` instruction
        CreateTransaction::create_transaction(context, create_args)?;

        Ok(())
    }
}
