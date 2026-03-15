use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::state::*;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::create_batch_create_message;

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
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump
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

    /// CHECK: Verified via verify_signer (native, session key, or external)
    pub creator: AccountInfo<'info>,

    /// The payer for the batch account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl CreateBatch<'_> {
    fn validate(
        &mut self,
        args: &CreateBatchArgs,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        let Self {
            settings,
            creator,
            ..
        } = self;

        // Build message for external signer verification
        let message = create_batch_create_message(
            &settings.key(),
            creator.key(),
            args.account_index,
        );

        // Verify signer (native, session key, or external) and check Initiate permission
        settings.verify_signer(
            creator,
            remaining_accounts,
            message,
            extra_verification_data.as_ref(),
            Some(Permission::Initiate),
        )?;

        settings.validate_account_index_unlocked(args.account_index)?;

        Ok(())
    }

    /// Create a new batch.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, None))]
    pub fn create_batch(ctx: Context<Self>, args: CreateBatchArgs) -> Result<()> {
        Self::create_batch_inner(ctx, args)
    }

    /// Create a new batch with V2 signer support.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, extra_verification_data))]
    pub fn create_batch_v2(ctx: Context<Self>, args: CreateBatchArgs, extra_verification_data: Option<ExtraVerificationData>) -> Result<()> {
        Self::create_batch_inner(ctx, args)
    }

    fn create_batch_inner(ctx: Context<Self>, args: CreateBatchArgs) -> Result<()> {
        let settings = &mut ctx.accounts.settings;
        let creator = &mut ctx.accounts.creator;
        let batch = &mut ctx.accounts.batch;
        let rent_payer = &mut ctx.accounts.rent_payer;
        let settings_key = settings.key();

        // Increment the transaction index.
        let index = settings
            .transaction_index
            .checked_add(1)
            .expect("overflow");

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
        batch.rent_collector = rent_payer.key();
        batch.index = index;
        batch.bump = ctx.bumps.batch;
        batch.account_index = args.account_index;
        batch.account_bump = smart_account_bump;
        batch.size = 0;
        batch.executed_transaction_index = 0;

        batch.invariant()?;

        // Updated last transaction index in the consensus account.
        settings.set_transaction_index(index)?;

        settings.invariant()?;

        // Logs for indexing.
        msg!("batch index: {}", index);

        Ok(())
    }
}
