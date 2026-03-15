use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::MAX_BUFFER_SIZE;
use crate::state::*;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::create_transaction_buffer_create_message;

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
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    #[account(
        init,
        payer = rent_payer,
        space = TransactionBuffer::size(args.final_buffer_size)?,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION_BUFFER,
            creator.key().as_ref(),
            &args.buffer_index.to_le_bytes(),
        ],
        bump
    )]
    pub transaction_buffer: Account<'info, TransactionBuffer>,

    /// CHECK: Verified via verify_signer (native, session key, or external)
    pub creator: AccountInfo<'info>,

    /// The payer for the transaction account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl CreateTransactionBuffer<'_> {
    fn validate(
        &mut self,
        args: &CreateTransactionBufferArgs,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        let Self {
            consensus_account, creator, ..
        } = self;

        // Build message for external signer verification
        let message = create_transaction_buffer_create_message(
            &consensus_account.key(),
            creator.key(),
            args.buffer_index,
            args.account_index,
            &args.final_buffer_hash,
            args.final_buffer_size,
        );

        // Verify signer (native, session key, or external) and check Initiate permission
        consensus_account.verify_signer(
            creator,
            remaining_accounts,
            message,
            extra_verification_data.as_ref(),
            Some(Permission::Initiate),
        )?;

        // Final Buffer Size must not exceed 4000 bytes
        require!(
            args.final_buffer_size as usize <= MAX_BUFFER_SIZE,
            SmartAccountError::FinalBufferSizeExceeded
        );

        Ok(())
    }

    /// Create a new transaction buffer.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, None))]
    pub fn create_transaction_buffer(
        ctx: Context<Self>,
        args: CreateTransactionBufferArgs,
    ) -> Result<()> {
        Self::create_transaction_buffer_inner(ctx, args)
    }

    /// Create a new transaction buffer with V2 signer support.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, extra_verification_data))]
    pub fn create_transaction_buffer_v2(
        ctx: Context<Self>,
        args: CreateTransactionBufferArgs,
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        Self::create_transaction_buffer_inner(ctx, args)
    }

    fn create_transaction_buffer_inner(
        ctx: Context<Self>,
        args: CreateTransactionBufferArgs,
    ) -> Result<()> {

        // Readonly Accounts
        let transaction_buffer = &mut ctx.accounts.transaction_buffer;
        let consensus_account = &ctx.accounts.consensus_account;
        let creator = &mut ctx.accounts.creator;

        // Get the buffer index.
        let buffer_index = args.buffer_index;

        // Initialize the transaction fields.
        transaction_buffer.settings = consensus_account.key();
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
