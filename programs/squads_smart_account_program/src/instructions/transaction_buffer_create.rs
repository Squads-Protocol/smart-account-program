use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::MAX_BUFFER_SIZE;
use crate::state::*;
use crate::utils::{
    create_transaction_buffer_create_message, verify_v2_context,
};

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

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateTransactionBufferV2Args {
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
    /// The key (Native) or key_id (External) of the creator
    pub creator_key: Pubkey,
    /// Client data params for WebAuthn verification (required for WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
}

#[derive(Accounts)]
#[instruction(args: CreateTransactionBufferArgs)]
pub struct CreateTransactionBuffer<'info> {
    #[account(
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

    /// The signer on the smart account that is creating the transaction.
    pub creator: Signer<'info>,

    /// The payer for the transaction account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl CreateTransactionBuffer<'_> {
    fn validate(&self, args: &CreateTransactionBufferArgs) -> Result<()> {
        validate_create_transaction_buffer(
            &self.consensus_account,
            args.final_buffer_size,
            self.creator.key(),
        )
    }

    /// Create a new transaction buffer.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn create_transaction_buffer(
        ctx: Context<Self>,
        args: CreateTransactionBufferArgs,
    ) -> Result<()> {
        create_transaction_buffer_inner(
            &mut ctx.accounts.transaction_buffer,
            &ctx.accounts.consensus_account,
            ctx.accounts.creator.key(),
            args.account_index,
            args.buffer_index,
            args.final_buffer_hash,
            args.final_buffer_size,
            args.buffer,
        )
    }
}

#[derive(Accounts)]
#[instruction(args: CreateTransactionBufferV2Args)]
pub struct CreateTransactionBufferV2<'info> {
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
            args.creator_key.as_ref(),
            &args.buffer_index.to_le_bytes(),
        ],
        bump
    )]
    pub transaction_buffer: Account<'info, TransactionBuffer>,

    /// The payer for the transaction account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl CreateTransactionBufferV2<'_> {
    fn validate(&self, args: &CreateTransactionBufferV2Args) -> Result<()> {
        validate_create_transaction_buffer(
            &self.consensus_account,
            args.final_buffer_size,
            args.creator_key,
        )
    }

    /// Create a new transaction buffer with V2 signer support.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn create_transaction_buffer_v2(
        ctx: Context<Self>,
        args: CreateTransactionBufferV2Args,
    ) -> Result<()> {
        let expected_message = create_transaction_buffer_create_message(
            &ctx.accounts.consensus_account.key(),
            &args.creator_key,
            args.buffer_index,
            args.account_index,
            &args.final_buffer_hash,
            args.final_buffer_size,
        );

        verify_v2_context(
            &mut ctx.accounts.consensus_account,
            args.creator_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        create_transaction_buffer_inner(
            &mut ctx.accounts.transaction_buffer,
            &ctx.accounts.consensus_account,
            args.creator_key,
            args.account_index,
            args.buffer_index,
            args.final_buffer_hash,
            args.final_buffer_size,
            args.buffer,
        )
    }
}

fn validate_create_transaction_buffer(
    consensus_account: &InterfaceAccount<ConsensusAccount>,
    final_buffer_size: u16,
    creator_key: Pubkey,
) -> Result<()> {
    // creator is a signer on the smart account
    require!(
        consensus_account.is_signer(creator_key).is_some(),
        SmartAccountError::NotASigner
    );
    // creator has initiate permissions
    require!(
        consensus_account.signer_has_permission(creator_key, Permission::Initiate),
        SmartAccountError::Unauthorized
    );

    // Final Buffer Size must not exceed 4000 bytes
    require!(
        final_buffer_size as usize <= MAX_BUFFER_SIZE,
        SmartAccountError::FinalBufferSizeExceeded
    );
    Ok(())
}

fn create_transaction_buffer_inner(
    transaction_buffer: &mut Account<TransactionBuffer>,
    consensus_account: &InterfaceAccount<ConsensusAccount>,
    creator_key: Pubkey,
    account_index: u8,
    buffer_index: u8,
    final_buffer_hash: [u8; 32],
    final_buffer_size: u16,
    buffer: Vec<u8>,
) -> Result<()> {
    transaction_buffer.settings = consensus_account.key();
    transaction_buffer.creator = creator_key;
    transaction_buffer.account_index = account_index;
    transaction_buffer.buffer_index = buffer_index;
    transaction_buffer.final_buffer_hash = final_buffer_hash;
    transaction_buffer.final_buffer_size = final_buffer_size;
    transaction_buffer.buffer = buffer;

    transaction_buffer.invariant()?;

    Ok(())
}
