use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::state::*;
use crate::utils::{create_batch_create_message, verify_v2_context};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateBatchArgs {
    /// Index of the smart account this batch belongs to.
    pub account_index: u8,
    pub memo: Option<String>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateBatchV2Args {
    /// Index of the smart account this batch belongs to.
    pub account_index: u8,
    pub memo: Option<String>,
    /// The key (Native) or key_id (External) of the creator
    pub creator_key: Pubkey,
    /// Client data params for WebAuthn verification (required for WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
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

    /// The signer of the settings that is creating the batch.
    pub creator: Signer<'info>,

    /// The payer for the batch account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl CreateBatch<'_> {
    fn validate(&self) -> Result<()> {
        let Self { settings, creator, .. } = self;

        Self::validate_signer(settings, creator.key())
    }

    fn validate_signer(settings: &Settings, signer_key: Pubkey) -> Result<()> {
        require!(
            settings.is_signer(signer_key).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            settings.signer_has_permission(signer_key, Permission::Initiate),
            SmartAccountError::Unauthorized
        );

        Ok(())
    }

    fn create_batch_inner(
        settings: &mut Settings,
        batch: &mut Batch,
        rent_payer: &Signer,
        creator_key: Pubkey,
        account_index: u8,
        batch_bump: u8,
        settings_key: Pubkey,
        program_id: &Pubkey,
    ) -> Result<()> {
        let index = settings
            .transaction_index
            .checked_add(1)
            .expect("overflow");

        let smart_account_seeds = &[
            SEED_PREFIX,
            settings_key.as_ref(),
            SEED_SMART_ACCOUNT,
            &account_index.to_le_bytes(),
        ];
        let (_, smart_account_bump) = Pubkey::find_program_address(smart_account_seeds, program_id);

        batch.settings = settings_key;
        batch.creator = creator_key;
        batch.rent_collector = rent_payer.key();
        batch.index = index;
        batch.bump = batch_bump;
        batch.account_index = account_index;
        batch.account_bump = smart_account_bump;
        batch.size = 0;
        batch.executed_transaction_index = 0;

        batch.invariant()?;

        settings.set_transaction_index(index)?;
        settings.invariant()?;

        msg!("batch index: {}", index);

        Ok(())
    }

    /// Create a new batch.
    #[access_control(ctx.accounts.validate())]
    pub fn create_batch(ctx: Context<Self>, args: CreateBatchArgs) -> Result<()> {
        let settings_key = ctx.accounts.settings.key();

        Self::create_batch_inner(
            &mut ctx.accounts.settings,
            &mut ctx.accounts.batch,
            &ctx.accounts.rent_payer,
            ctx.accounts.creator.key(),
            args.account_index,
            ctx.bumps.batch,
            settings_key,
            ctx.program_id,
        )
    }

    #[access_control(ctx.accounts.validate_v2(&args))]
    pub fn create_batch_v2(ctx: Context<Self>, args: CreateBatchV2Args) -> Result<()> {
        let expected_message = create_batch_create_message(
            &ctx.accounts.settings.key(),
            args.creator_key,
            args.account_index,
        );

        verify_v2_context(
            &mut ctx.accounts.settings,
            args.creator_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        let settings_key = ctx.accounts.settings.key();

        Self::create_batch_inner(
            &mut ctx.accounts.settings,
            &mut ctx.accounts.batch,
            &ctx.accounts.rent_payer,
            args.creator_key,
            args.account_index,
            ctx.bumps.batch,
            settings_key,
            ctx.program_id,
        )
    }

    fn validate_v2(&self, args: &CreateBatchV2Args) -> Result<()> {
        Self::validate_signer(&self.settings, args.creator_key)
    }
}
