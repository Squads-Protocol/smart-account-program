use anchor_lang::prelude::*;

use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ExtendTransactionBufferArgs {
    // Buffer to extend the TransactionBuffer with.
    pub buffer: Vec<u8>,
}

#[derive(Accounts)]
#[instruction(args: ExtendTransactionBufferArgs)]
pub struct ExtendTransactionBuffer<'info> {
    #[account(
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    #[account(
        mut,
        // Only the creator can extend the buffer
        constraint = transaction_buffer.creator == creator.key() @ SmartAccountError::Unauthorized,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION_BUFFER,
            creator.key().as_ref(),
            &transaction_buffer.buffer_index.to_le_bytes()
        ],
        bump
    )]
    pub transaction_buffer: Account<'info, TransactionBuffer>,

    /// The signer on the smart account that created the TransactionBuffer.
    pub creator: Signer<'info>,
}

impl ExtendTransactionBuffer<'_> {
    fn validate(&self, args: &ExtendTransactionBufferArgs) -> Result<()> {
        let Self {
            consensus_account,
            creator,
            transaction_buffer,
            ..
        } = self;

        // creator is still a signer on the smart account
        require!(
            consensus_account.is_signer(creator.key()).is_some(),
            SmartAccountError::NotASigner
        );

        // creator still has initiate permissions
        require!(
            consensus_account.signer_has_permission(creator.key(), Permission::Initiate),
            SmartAccountError::Unauthorized
        );

        // Extended Buffer size must not exceed final buffer size
        // Calculate remaining space in the buffer
        let current_buffer_size = transaction_buffer.buffer.len() as u16;
        let remaining_space = transaction_buffer
            .final_buffer_size
            .checked_sub(current_buffer_size)
            .unwrap();

        // Check if the new data exceeds the remaining space
        let new_data_size = args.buffer.len() as u16;
        require!(
            new_data_size <= remaining_space,
            SmartAccountError::FinalBufferSizeExceeded
        );

        Ok(())
    }

    /// Extend the transaction buffer with the provided buffer.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn extend_transaction_buffer(
        ctx: Context<Self>,
        args: ExtendTransactionBufferArgs,
    ) -> Result<()> {
        // Mutable Accounts
        let transaction_buffer = &mut ctx.accounts.transaction_buffer;

        // Required Data
        let buffer_slice_extension = args.buffer;

        // Extend the buffer inside the transaction buffer
        transaction_buffer
            .buffer
            .extend_from_slice(&buffer_slice_extension);

        // Invariant function on the transaction buffer
        transaction_buffer.invariant()?;

        Ok(())
    }
}
