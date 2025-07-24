use anchor_lang::prelude::*;

use crate::consensus_trait::ConsensusAccountType;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::*;
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
    /// Consensus account this batch belongs to.
    #[account(
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok(),
        // Batches currenlty don't support policies
        constraint = consensus_account.account_type() == ConsensusAccountType::Settings
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    /// The proposal account associated with the batch.
    #[account(
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
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
            consensus_account.key().as_ref(),
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
            consensus_account.key().as_ref(),
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
    fn validate(&self, ctx: &Context<Self>) -> Result<()> {
        let Self {
            consensus_account,
            signer,
            proposal,
            batch,
            ..
        } = self;

        // Check if the consensus account is active
        consensus_account.is_active(&ctx.remaining_accounts)?;

        // `signer`
        require!(
            consensus_account.is_signer(signer.key()).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            consensus_account.signer_has_permission(signer.key(), Permission::Initiate),
            SmartAccountError::Unauthorized
        );
        // Only batch creator can add transactions to it.
        require!(
            signer.key() == batch.creator,
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
    #[access_control(ctx.accounts.validate(&ctx))]
    pub fn add_transaction_to_batch(ctx: Context<Self>, args: AddTransactionToBatchArgs) -> Result<()> {
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
