use anchor_lang::prelude::*;
use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::state::*;
use crate::utils::{create_batch_add_transaction_message, verify_v2_context};
use crate::TransactionMessage;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddTransactionToBatchArgs {
    /// Number of ephemeral signing PDAs required by the transaction.
    pub ephemeral_signers: u8,
    pub transaction_message: Vec<u8>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddTransactionToBatchV2Args {
    /// Number of ephemeral signing PDAs required by the transaction.
    pub ephemeral_signers: u8,
    pub transaction_message: Vec<u8>,
    /// The key (Native) or key_id (External) of the signer
    pub signer_key: Pubkey,
    /// Client data params for WebAuthn verification (required for WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
}

#[derive(Accounts)]
#[instruction(args: AddTransactionToBatchArgs)]
pub struct AddTransactionToBatch<'info> {
    /// Settings account this batch belongs to.
    #[account(
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

    /// Signer of the smart account.
    pub signer: Signer<'info>,

    /// The payer for the batch transaction account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl AddTransactionToBatch<'_> {
    fn validate(&self) -> Result<()> {
        let Self {
            settings,
            signer,
            proposal,
            batch,
            ..
        } = self;

        Self::validate_signer(settings, signer.key(), batch.creator)?;

        // `proposal`
        require!(
            matches!(proposal.status, ProposalStatus::Draft { .. }),
            SmartAccountError::InvalidProposalStatus
        );

        // `batch` is validated by its seeds.

        Ok(())
    }

    fn validate_signer(
        settings: &Settings,
        signer_key: Pubkey,
        batch_creator: Pubkey,
    ) -> Result<()> {
        require!(
            settings.is_signer(signer_key).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            settings.signer_has_permission(signer_key, Permission::Initiate),
            SmartAccountError::Unauthorized
        );
        require!(signer_key == batch_creator, SmartAccountError::Unauthorized);

        Ok(())
    }

    fn add_transaction_inner(
        batch: &mut Batch,
        transaction: &mut BatchTransaction,
        rent_payer: &Signer,
        args: AddTransactionToBatchArgs,
        batch_key: Pubkey,
        program_id: &Pubkey,
        transaction_bump: u8,
    ) -> Result<()> {
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

                let (_, bump) = Pubkey::find_program_address(ephemeral_signer_seeds, program_id);

                bump
            })
            .collect();

        transaction.bump = transaction_bump;
        transaction.rent_collector = rent_payer.key();
        transaction.ephemeral_signer_bumps = ephemeral_signer_bumps;
        transaction.message = transaction_message.try_into()?;

        batch.size = batch.size.checked_add(1).expect("overflow");

        msg!("batch index: {}", batch.index);
        msg!("batch size: {}", batch.size);

        Ok(())
    }

    /// Add a transaction to the batch.
    #[access_control(ctx.accounts.validate())]
    pub fn add_transaction_to_batch(ctx: Context<Self>, args: AddTransactionToBatchArgs) -> Result<()> {
        let batch_key = ctx.accounts.batch.key();

        Self::add_transaction_inner(
            &mut ctx.accounts.batch,
            &mut ctx.accounts.transaction,
            &ctx.accounts.rent_payer,
            args,
            batch_key,
            ctx.program_id,
            ctx.bumps.transaction,
        )
    }

    #[access_control(ctx.accounts.validate_v2(&args))]
    pub fn add_transaction_to_batch_v2(
        ctx: Context<Self>,
        args: AddTransactionToBatchV2Args,
    ) -> Result<()> {
        let expected_message = create_batch_add_transaction_message(
            &ctx.accounts.batch.key(),
            args.signer_key,
            u64::from(ctx.accounts.batch.size.checked_add(1).unwrap()),
        );

        verify_v2_context(
            &mut ctx.accounts.settings,
            args.signer_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        let batch_key = ctx.accounts.batch.key();

        Self::add_transaction_inner(
            &mut ctx.accounts.batch,
            &mut ctx.accounts.transaction,
            &ctx.accounts.rent_payer,
            AddTransactionToBatchArgs {
                ephemeral_signers: args.ephemeral_signers,
                transaction_message: args.transaction_message,
            },
            batch_key,
            ctx.program_id,
            ctx.bumps.transaction,
        )
    }

    fn validate_v2(&self, args: &AddTransactionToBatchV2Args) -> Result<()> {
        Self::validate_signer(&self.settings, args.signer_key, self.batch.creator)?;

        require!(
            matches!(self.proposal.status, ProposalStatus::Draft { .. }),
            SmartAccountError::InvalidProposalStatus
        );

        Ok(())
    }
}
