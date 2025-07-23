use anchor_lang::prelude::*;

use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::*;

#[derive(Accounts)]
pub struct CloseTransactionBuffer<'info> {
    #[account(
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    #[account(
        mut,
        // Rent gets returned to the creator
        close = creator,
        // Only the creator can close the buffer
        constraint = transaction_buffer.creator == creator.key() @ SmartAccountError::Unauthorized,
        // Account can be closed anytime by the creator, regardless of the
        // current settings transaction index
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

impl CloseTransactionBuffer<'_> {
    fn validate(&self) -> Result<()> {
        Ok(())
    }

    /// Close a transaction buffer account.
    #[access_control(ctx.accounts.validate())]
    pub fn close_transaction_buffer(ctx: Context<Self>) -> Result<()> {
        Ok(())
    }
}
