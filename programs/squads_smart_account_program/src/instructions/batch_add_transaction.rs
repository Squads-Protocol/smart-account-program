use anchor_lang::prelude::*;
use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::state::*;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::create_batch_add_transaction_message;
use crate::TransactionMessage;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddTransactionToBatchArgs {
    /// Number of ephemeral signing PDAs required by the transaction.
    pub ephemeral_signers: u8,
    pub transaction_message: Vec<u8>,
}

#[derive(Accounts)]
#[instruction(args: AddTransactionToBatchArgs)]
pub struct AddTransactionToBatch<'info> {
    /// Settings account this batch belongs to.
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump
    )]
    pub settings: Account<'info, Settings>,

    /// The proposal account associated with the batch.
    #[account(
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &batch.index.to_le_bytes(),
            SEED_PROPOSAL,
        ],
        bump = proposal.bump,
    )]
    pub proposal: Account<'info, Proposal>,

    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &batch.index.to_le_bytes(),
        ],
        bump = batch.bump,
    )]
    pub batch: Account<'info, Batch>,

    /// `BatchTransaction` account to initialize and add to the `batch`.
    #[account(
        init,
        payer = rent_payer,
        space = BatchTransaction::size(args.ephemeral_signers, &args.transaction_message)?,
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &batch.index.to_le_bytes(),
            SEED_BATCH_TRANSACTION,
            &batch.size.checked_add(1).unwrap().to_le_bytes(),
        ],
        bump
    )]
    pub transaction: Account<'info, BatchTransaction>,

    /// CHECK: Verified via verify_signer (native, session key, or external)
    pub signer: AccountInfo<'info>,

    /// The payer for the batch transaction account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl AddTransactionToBatch<'_> {
    fn validate(
        &mut self,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        let Self {
            settings,
            signer,
            proposal,
            batch,
            ..
        } = self;

        // Build message for external signer verification
        let message = create_batch_add_transaction_message(
            &batch.key(),
            signer.key(),
            batch.index,
        );

        // Verify signer (native, session key, or external) and check Initiate permission
        let canonical_key = settings.verify_signer(
            signer,
            remaining_accounts,
            message,
            extra_verification_data.as_ref(),
            Some(Permission::Initiate),
        )?;

        // Only batch creator can add transactions to it (using canonical key).
        require!(
            canonical_key == batch.creator,
            SmartAccountError::Unauthorized
        );

        // `proposal`
        require!(
            matches!(proposal.status, ProposalStatus::Draft { .. }),
            SmartAccountError::InvalidProposalStatus
        );

        // `batch` is validated by its seeds.

        Ok(())
    }

    /// Add a transaction to the batch.
    #[access_control(ctx.accounts.validate(&ctx.remaining_accounts, None))]
    pub fn add_transaction_to_batch(ctx: Context<Self>, args: AddTransactionToBatchArgs) -> Result<()> {
        Self::add_transaction_to_batch_inner(ctx, args)
    }

    /// Add a transaction to the batch with V2 signer support.
    #[access_control(ctx.accounts.validate(&ctx.remaining_accounts, extra_verification_data))]
    pub fn add_transaction_to_batch_v2(ctx: Context<Self>, args: AddTransactionToBatchArgs, extra_verification_data: Option<ExtraVerificationData>) -> Result<()> {
        Self::add_transaction_to_batch_inner(ctx, args)
    }

    fn add_transaction_to_batch_inner(ctx: Context<Self>, args: AddTransactionToBatchArgs) -> Result<()> {
        let batch = &mut ctx.accounts.batch;
        let transaction = &mut ctx.accounts.transaction;
        let rent_payer = &mut ctx.accounts.rent_payer;
        let batch_key = batch.key();

        let transaction_message =
            TransactionMessage::deserialize(&mut args.transaction_message.as_slice())?;

        let ephemeral_signer_bumps: Vec<u8> = (0..args.ephemeral_signers)
            .map(|ephemeral_signer_index| {
                let ephemeral_signer_seeds = &[
                    SEED_PREFIX,
                    batch_key.as_ref(),
                    SEED_EPHEMERAL_SIGNER,
                    &ephemeral_signer_index.to_le_bytes(),
                ];

                let (_, bump) =
                    Pubkey::find_program_address(ephemeral_signer_seeds, ctx.program_id);

                bump
            })
            .collect();

        transaction.bump = ctx.bumps.transaction;
        transaction.rent_collector = rent_payer.key();
        transaction.ephemeral_signer_bumps = ephemeral_signer_bumps;
        transaction.message = transaction_message.try_into()?;

        // Increment the batch size.
        batch.size = batch.size.checked_add(1).expect("overflow");

        // Logs for indexing.
        msg!("batch index: {}", batch.index);
        msg!("batch size: {}", batch.size);

        Ok(())
    }
}
