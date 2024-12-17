use anchor_lang::prelude::*;

use crate::errors::*;
use crate::state::MAX_BUFFER_SIZE;
use crate::state::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateTransactionBufferArgs {
    /// Index of the buffer account to seed the account derivation
    pub buffer_index: u8,
    /// Index of the smart account this transaction belongs to.
    pub account_index: u8,
    /// Hash of the final assembled transaction message.
    pub final_buffer_hash: [u8; 32],
    /// Final size of the buffer.
    pub final_buffer_size: u16,
    /// Initial slice of the buffer.
    pub buffer: Vec<u8>,
}

#[derive(Accounts)]
#[instruction(args: CreateTransactionBufferArgs)]
pub struct CreateTransactionBuffer<'info> {
    #[account(
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    #[account(
        init,
        payer = rent_payer,
        space = TransactionBuffer::size(args.final_buffer_size)?,
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION_BUFFER,
            creator.key().as_ref(),
            &args.buffer_index.to_le_bytes(),
        ],
        bump
    )]
    pub transaction_buffer: Account<'info, TransactionBuffer>,

    /// The signer on the smart account that is creating the transaction.
    pub creator: Signer<'info>,

    /// The payer for the transaction account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl CreateTransactionBuffer<'_> {
    fn validate(&self, args: &CreateTransactionBufferArgs) -> Result<()> {
        let Self {
            settings, creator, ..
        } = self;

        // creator is a signer on the smart account
        require!(
            settings.is_signer(creator.key()).is_some(),
            SmartAccountError::NotASigner
        );
        // creator has initiate permissions
        require!(
            settings.signer_has_permission(creator.key(), Permission::Initiate),
            SmartAccountError::Unauthorized
        );

        // Final Buffer Size must not exceed 4000 bytes
        require!(
            args.final_buffer_size as usize <= MAX_BUFFER_SIZE,
            SmartAccountError::FinalBufferSizeExceeded
        );
        Ok(())
    }

    /// Create a new transaction buffer.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn create_transaction_buffer(
        ctx: Context<Self>,
        args: CreateTransactionBufferArgs,
    ) -> Result<()> {

        // Readonly Accounts
        let transaction_buffer = &mut ctx.accounts.transaction_buffer;
        let settings = &ctx.accounts.settings;
        let creator = &mut ctx.accounts.creator;

        // Get the buffer index.    
        let buffer_index = args.buffer_index;

        // Initialize the transaction fields.
        transaction_buffer.settings = settings.key();
        transaction_buffer.creator = creator.key();
        transaction_buffer.account_index = args.account_index;
        transaction_buffer.buffer_index = buffer_index;
        transaction_buffer.final_buffer_hash = args.final_buffer_hash;
        transaction_buffer.final_buffer_size = args.final_buffer_size;
        transaction_buffer.buffer = args.buffer;

        // Invariant function on the transaction buffer
        transaction_buffer.invariant()?;

        Ok(())
    }
}
