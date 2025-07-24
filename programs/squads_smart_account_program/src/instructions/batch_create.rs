use anchor_lang::prelude::*;

use crate::consensus_trait::ConsensusAccountType;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateBatchArgs {
    /// Index of the smart account this batch belongs to.
    pub account_index: u8,
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct CreateBatch<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok(),
        // Batches currenlty don't support policies
        constraint = consensus_account.account_type() == ConsensusAccountType::Settings
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    #[account(
        init,
        payer = rent_payer,
        space = 8 + Batch::INIT_SPACE,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION,
            &consensus_account.transaction_index().checked_add(1).unwrap().to_le_bytes(),
        ],
        bump
    )]
    pub batch: Account<'info, Batch>,

    /// The signer of the settings that is creating the batch.
    pub creator: Signer<'info>,

    /// The payer for the batch account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl CreateBatch<'_> {
    fn validate(&self, ctx: &Context<Self>) -> Result<()> {
        let Self {
            consensus_account,
            creator,
            ..
        } = self;

        // Check if the consensus account is active
        consensus_account.is_active(&ctx.remaining_accounts)?;

        // creator
        require!(
            consensus_account.is_signer(creator.key()).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            consensus_account.signer_has_permission(creator.key(), Permission::Initiate),
            SmartAccountError::Unauthorized
        );

        Ok(())
    }

    /// Create a new batch.
    #[access_control(ctx.accounts.validate(&ctx))]
    pub fn create_batch(ctx: Context<Self>, args: CreateBatchArgs) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let creator = &mut ctx.accounts.creator;
        let batch = &mut ctx.accounts.batch;
        let rent_payer = &mut ctx.accounts.rent_payer;
        let consensus_account_key = consensus_account.key();

        // Increment the transaction index.
        let index = consensus_account
            .transaction_index()
            .checked_add(1)
            .expect("overflow");

        let smart_account_seeds = &[
            SEED_PREFIX,
            consensus_account_key.as_ref(),
            SEED_SMART_ACCOUNT,
            &args.account_index.to_le_bytes(),
        ];
        let (_, smart_account_bump) =
            Pubkey::find_program_address(smart_account_seeds, ctx.program_id);

        batch.settings = consensus_account_key;
        batch.creator = creator.key();
        batch.rent_collector = rent_payer.key();
        batch.index = index;
        batch.bump = ctx.bumps.batch;
        batch.account_index = args.account_index;
        batch.account_bump = smart_account_bump;
        batch.size = 0;
        batch.executed_transaction_index = 0;

        batch.invariant()?;

        // Updated last transaction index in the consensus account.
        consensus_account.set_transaction_index(index)?;

        consensus_account.invariant()?;

        // Logs for indexing.
        msg!("batch index: {}", index);

        Ok(())
    }
}
