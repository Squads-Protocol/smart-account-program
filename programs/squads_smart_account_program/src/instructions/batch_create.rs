use anchor_lang::prelude::*;

use crate::errors::*;
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
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    #[account(
        init,
        payer = rent_payer,
        space = 8 + Batch::INIT_SPACE,
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &settings.transaction_index.checked_add(1).unwrap().to_le_bytes(),
        ],
        bump
    )]
    pub batch: Account<'info, Batch>,

    /// The member of the multisig that is creating the batch.
    pub creator: Signer<'info>,

    /// The payer for the batch account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl CreateBatch<'_> {
    fn validate(&self) -> Result<()> {
        let Self {
            settings, creator, ..
        } = self;

        // creator
        require!(
            settings.is_signer(creator.key()).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            settings.signer_has_permission(creator.key(), Permission::Initiate),
            SmartAccountError::Unauthorized
        );

        Ok(())
    }

    /// Create a new batch.
    #[access_control(ctx.accounts.validate())]
    pub fn create_batch(ctx: Context<Self>, args: CreateBatchArgs) -> Result<()> {
        let settings = &mut ctx.accounts.settings;
        let creator = &mut ctx.accounts.creator;
        let batch = &mut ctx.accounts.batch;

        let settings_key = settings.key();

        // Increment the transaction index.
        let index = settings.transaction_index.checked_add(1).expect("overflow");

        let smart_account_seeds = &[
            SEED_PREFIX,
            settings_key.as_ref(),
            SEED_SMART_ACCOUNT,
            &args.account_index.to_le_bytes(),
        ];
        let (_, smart_account_bump) =
            Pubkey::find_program_address(smart_account_seeds, ctx.program_id);

        batch.settings = settings_key;
        batch.creator = creator.key();
        batch.index = index;
        batch.bump = ctx.bumps.batch;
        batch.account_index = args.account_index;
        batch.account_bump = smart_account_bump;
        batch.size = 0;
        batch.executed_transaction_index = 0;

        batch.invariant()?;

        // Updated last transaction index in the multisig account.
        settings.transaction_index = index;

        settings.invariant()?;

        // Logs for indexing.
        msg!("batch index: {}", index);

        Ok(())
    }
}
