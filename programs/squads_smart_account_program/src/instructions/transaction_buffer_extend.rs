use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::*;
use crate::instructions::*;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::create_transaction_buffer_extend_message;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ExtendTransactionBufferArgs {
    // Buffer to extend the TransactionBuffer with.
    pub buffer: Vec<u8>,
}

#[derive(Accounts)]
#[instruction(args: ExtendTransactionBufferArgs)]
pub struct ExtendTransactionBuffer<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    #[account(
        mut,
        // PDA derived from stored creator (canonical key for V2, raw key for V1)
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION_BUFFER,
            transaction_buffer.creator.as_ref(),
            &transaction_buffer.buffer_index.to_le_bytes()
        ],
        bump
    )]
    pub transaction_buffer: Account<'info, TransactionBuffer>,

    pub creator: ResolvedSigner<'info>,
}

impl ExtendTransactionBuffer<'_> {
    fn validate(
        &mut self,
        args: &ExtendTransactionBufferArgs,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        let Self {
            consensus_account,
            creator,
            transaction_buffer,
            ..
        } = self;

        // Build message for external signer verification
        let message = create_transaction_buffer_extend_message(
            &transaction_buffer.key(),
            &args.buffer,
        );

        // Resolve and verify signer (native, session key, or external) and check Initiate permission
        creator.verify(
            &mut **consensus_account,
            remaining_accounts,
            message,
            extra_verification_data.as_ref(),
            Some(Permission::Initiate),
        )?;

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
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, None))]
    pub fn extend_transaction_buffer(
        ctx: Context<Self>,
        args: ExtendTransactionBufferArgs,
    ) -> Result<()> {
        // V1: raw key must match stored creator
        require!(
            ctx.accounts.transaction_buffer.creator == ctx.accounts.creator.key(),
            SmartAccountError::Unauthorized
        );
        Self::extend_transaction_buffer_inner(ctx, args)
    }

    /// Extend the transaction buffer with V2 signer support.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, extra_verification_data))]
    pub fn extend_transaction_buffer_v2(
        ctx: Context<Self>,
        args: ExtendTransactionBufferArgs,
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        // V2: resolved key must match stored creator (session keys resolve to parent)
        let resolved_key = ctx.accounts.creator.resolved_key()?;
        require!(
            ctx.accounts.transaction_buffer.creator == resolved_key,
            SmartAccountError::Unauthorized
        );
        Self::extend_transaction_buffer_inner(ctx, args)
    }

    fn extend_transaction_buffer_inner(
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
