use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::*;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::create_transaction_buffer_close_message;

#[derive(Accounts)]
pub struct CloseTransactionBuffer<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    #[account(
        mut,
        close = creator,
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

    /// CHECK: Verified in validate. Uses AccountInfo (not ResolvedSigner) because
    /// Anchor's `close = creator` requires the target to support mut constraints.
    pub creator: AccountInfo<'info>,
}

impl CloseTransactionBuffer<'_> {
    fn validate(
        &mut self,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        // Fast path: native signer whose key directly matches stored creator.
        // No membership check — allows removed members to reclaim buffer rent.
        if self.creator.is_signer && self.creator.key() == self.transaction_buffer.creator {
            return Ok(());
        }

        // Slow path: session key or external signer — full verification required.
        let message = create_transaction_buffer_close_message(
            &self.transaction_buffer.key(),
            &self.consensus_account.key(),
        );

        self.consensus_account.verify_signer(
            &self.creator,
            remaining_accounts,
            message,
            extra_verification_data.as_ref(),
            None,
        )?;

        Ok(())
    }

    /// Close a transaction buffer account.
    #[access_control(ctx.accounts.validate(&ctx.remaining_accounts, None))]
    pub fn close_transaction_buffer(ctx: Context<Self>) -> Result<()> {
        require!(
            ctx.accounts.transaction_buffer.creator == ctx.accounts.creator.key(),
            SmartAccountError::Unauthorized
        );
        Ok(())
    }

    /// Close a transaction buffer account with V2 signer support.
    #[access_control(ctx.accounts.validate(&ctx.remaining_accounts, extra_verification_data))]
    pub fn close_transaction_buffer_v2(
        ctx: Context<Self>,
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        // Direct creator match — no membership needed
        if ctx.accounts.creator.key() == ctx.accounts.transaction_buffer.creator {
            return Ok(());
        }
        // Session key or external signer — use resolved key
        let resolved_key = ctx.accounts.consensus_account.resolve_signer_key(ctx.accounts.creator.key(), ctx.accounts.creator.is_signer)?;
        require!(
            ctx.accounts.transaction_buffer.creator == resolved_key,
            SmartAccountError::Unauthorized
        );
        Ok(())
    }
}
