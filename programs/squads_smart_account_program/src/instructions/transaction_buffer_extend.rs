use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::*;
use crate::utils::{create_transaction_buffer_extend_message, verify_v2_context};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ExtendTransactionBufferArgs {
    // Buffer to extend the TransactionBuffer with.
    pub buffer: Vec<u8>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ExtendTransactionBufferV2Args {
    /// Buffer to extend the TransactionBuffer with.
    pub buffer: Vec<u8>,
    /// The key (Native) or key_id (External) of the creator
    pub creator_key: Pubkey,
    /// Client data params for WebAuthn verification (required for WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
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
        validate_extend_transaction_buffer(
            &self.consensus_account,
            &self.transaction_buffer,
            self.creator.key(),
            &args.buffer,
        )
    }

    /// Extend the transaction buffer with the provided buffer.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn extend_transaction_buffer(
        ctx: Context<Self>,
        args: ExtendTransactionBufferArgs,
    ) -> Result<()> {
        extend_transaction_buffer_inner(&mut ctx.accounts.transaction_buffer, args.buffer)
    }
}

#[derive(Accounts)]
#[instruction(args: ExtendTransactionBufferV2Args)]
pub struct ExtendTransactionBufferV2<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    #[account(
        mut,
        // Only the creator can extend the buffer
        constraint = transaction_buffer.creator == args.creator_key @ SmartAccountError::Unauthorized,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION_BUFFER,
            args.creator_key.as_ref(),
            &transaction_buffer.buffer_index.to_le_bytes()
        ],
        bump
    )]
    pub transaction_buffer: Account<'info, TransactionBuffer>,
}

impl ExtendTransactionBufferV2<'_> {
    fn validate(&self, args: &ExtendTransactionBufferV2Args) -> Result<()> {
        validate_extend_transaction_buffer(
            &self.consensus_account,
            &self.transaction_buffer,
            args.creator_key,
            &args.buffer,
        )
    }

    /// Extend the transaction buffer with V2 signer support.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn extend_transaction_buffer_v2(
        ctx: Context<Self>,
        args: ExtendTransactionBufferV2Args,
    ) -> Result<()> {
        let expected_message = create_transaction_buffer_extend_message(
            &ctx.accounts.transaction_buffer.key(),
            &args.buffer,
        );

        verify_v2_context(
            &mut ctx.accounts.consensus_account,
            args.creator_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        extend_transaction_buffer_inner(&mut ctx.accounts.transaction_buffer, args.buffer)
    }
}

fn validate_extend_transaction_buffer(
    consensus_account: &InterfaceAccount<ConsensusAccount>,
    transaction_buffer: &TransactionBuffer,
    creator_key: Pubkey,
    buffer: &[u8],
) -> Result<()> {
    // creator is still a signer on the smart account
    require!(
        consensus_account.is_signer(creator_key).is_some(),
        SmartAccountError::NotASigner
    );

    // creator still has initiate permissions
    require!(
        consensus_account.signer_has_permission(creator_key, Permission::Initiate),
        SmartAccountError::Unauthorized
    );

    // Extended Buffer size must not exceed final buffer size
    let current_buffer_size = transaction_buffer.buffer.len() as u16;
    let remaining_space = transaction_buffer
        .final_buffer_size
        .checked_sub(current_buffer_size)
        .unwrap();

    let new_data_size = buffer.len() as u16;
    require!(
        new_data_size <= remaining_space,
        SmartAccountError::FinalBufferSizeExceeded
    );

    Ok(())
}

fn extend_transaction_buffer_inner(
    transaction_buffer: &mut Account<TransactionBuffer>,
    buffer_slice_extension: Vec<u8>,
) -> Result<()> {
    transaction_buffer
        .buffer
        .extend_from_slice(&buffer_slice_extension);

    transaction_buffer.invariant()?;

    Ok(())
}
