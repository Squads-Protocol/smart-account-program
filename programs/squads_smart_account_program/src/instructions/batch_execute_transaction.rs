use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::state::*;
use crate::utils::{
    create_batch_execute_transaction_message, derive_ephemeral_signers, split_instructions_sysvar,
    verify_v2_context, ExecutableTransactionMessage,
};

use super::transaction_execute::validate_proposal_execution_ready;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ExecuteBatchTransactionV2Args {
    /// The key (Native) or key_id (External) of the signer
    pub signer_key: Pubkey,
    /// Client data params for WebAuthn verification (required for WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
}

#[derive(Accounts)]
pub struct ExecuteBatchTransaction<'info> {
    /// Settings account this batch belongs to.
    #[account(
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump
    )]
    pub settings: Account<'info, Settings>,

    /// Signer of the settings.
    pub signer: Signer<'info>,

    /// The proposal account associated with the batch.
    /// If `transaction` is the last in the batch, the `proposal` status will be set to `Executed`.
    #[account(
        mut,
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

    /// Batch transaction to execute.
    #[account(
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &batch.index.to_le_bytes(),
            SEED_BATCH_TRANSACTION,
            &batch.executed_transaction_index.checked_add(1).unwrap().to_le_bytes(),
        ],
        bump = transaction.bump,
    )]
    pub transaction: Account<'info, BatchTransaction>,
    //
    // `remaining_accounts` must include the following accounts in the exact order:
    // 1. AddressLookupTable accounts in the order they appear in `message.address_table_lookups`.
    // 2. Accounts in the order they appear in `message.account_keys`.
    // 3. Accounts in the order they appear in `message.address_table_lookups`.
}

impl ExecuteBatchTransaction<'_> {
    fn validate(&self) -> Result<()> {
        let Self {
            settings,
            signer,
            proposal,
            ..
        } = self;

        Self::validate_signer(settings, signer.key())?;

        Self::validate_proposal(settings, proposal)?;
        // Stale batch transaction proposals CAN be executed if they were approved
        // before becoming stale, hence no check for staleness here.

        // `batch` is validated by its seeds.

        // `transaction` is validated by its seeds.

        Ok(())
    }

    fn validate_signer(settings: &Settings, signer_key: Pubkey) -> Result<()> {
        require!(
            settings.is_signer(signer_key).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            settings.signer_has_permission(signer_key, Permission::Execute),
            SmartAccountError::Unauthorized
        );

        Ok(())
    }

    fn validate_proposal(settings: &Settings, proposal: &Proposal) -> Result<()> {
        validate_proposal_execution_ready(settings.time_lock, proposal)
    }

    fn execute_inner<'info>(
        _settings: &mut Settings,
        proposal: &mut Proposal,
        batch: &mut Batch,
        transaction: BatchTransaction,
        remaining_accounts: &[AccountInfo<'info>],
        settings_key: Pubkey,
        batch_key: Pubkey,
        proposal_key: Pubkey,
        program_id: &Pubkey,
    ) -> Result<()> {
        let smart_account_seeds = &[
            SEED_PREFIX,
            settings_key.as_ref(),
            SEED_SMART_ACCOUNT,
            &batch.account_index.to_le_bytes(),
            &[batch.account_bump],
        ];

        let transaction_message = transaction.message;
        let num_lookups = transaction_message.address_table_lookups.len();

        let message_account_infos = remaining_accounts
            .get(num_lookups..)
            .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;
        let address_lookup_table_account_infos = remaining_accounts
            .get(..num_lookups)
            .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;

        let smart_account_pubkey =
            Pubkey::create_program_address(smart_account_seeds, program_id).unwrap();

        let (ephemeral_signer_keys, ephemeral_signer_seeds) =
            derive_ephemeral_signers(batch_key, &transaction.ephemeral_signer_bumps);

        let executable_message = ExecutableTransactionMessage::new_validated(
            transaction_message,
            message_account_infos,
            address_lookup_table_account_infos,
            &smart_account_pubkey,
            &ephemeral_signer_keys,
        )?;

        let protected_accounts = &[proposal_key, batch_key];

        executable_message.execute_message(
            smart_account_seeds,
            &ephemeral_signer_seeds,
            protected_accounts,
        )?;

        batch.executed_transaction_index = batch
            .executed_transaction_index
            .checked_add(1)
            .expect("overflow");

        if batch.executed_transaction_index == batch.size {
            proposal.status = ProposalStatus::Executed {
                timestamp: Clock::get()?.unix_timestamp,
            };
        }

        batch.invariant()?;

        Ok(())
    }

    /// Execute a transaction from the batch.
    #[access_control(ctx.accounts.validate())]
    pub fn execute_batch_transaction(ctx: Context<Self>) -> Result<()> {
        let settings_key = ctx.accounts.settings.key();
        let batch_key = ctx.accounts.batch.key();
        let proposal_key = ctx.accounts.proposal.key();
        let transaction = ctx.accounts.transaction.take();

        Self::execute_inner(
            &mut ctx.accounts.settings,
            &mut ctx.accounts.proposal,
            &mut ctx.accounts.batch,
            transaction,
            &ctx.remaining_accounts,
            settings_key,
            batch_key,
            proposal_key,
            ctx.program_id,
        )
    }

    #[access_control(ctx.accounts.validate_v2(&args))]
    pub fn execute_batch_transaction_v2(
        ctx: Context<Self>,
        args: ExecuteBatchTransactionV2Args,
    ) -> Result<()> {
        let expected_message = create_batch_execute_transaction_message(
            &ctx.accounts.batch.key(),
            args.signer_key,
            u64::from(
                ctx.accounts
                    .batch
                    .executed_transaction_index
                    .checked_add(1)
                    .unwrap(),
            ),
        );

        verify_v2_context(
            &mut ctx.accounts.settings,
            args.signer_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        let (_, remaining_accounts) = split_instructions_sysvar(&ctx.remaining_accounts);

        let settings_key = ctx.accounts.settings.key();
        let batch_key = ctx.accounts.batch.key();
        let proposal_key = ctx.accounts.proposal.key();
        let transaction = ctx.accounts.transaction.take();

        Self::execute_inner(
            &mut ctx.accounts.settings,
            &mut ctx.accounts.proposal,
            &mut ctx.accounts.batch,
            transaction,
            remaining_accounts,
            settings_key,
            batch_key,
            proposal_key,
            ctx.program_id,
        )
    }

    fn validate_v2(&self, args: &ExecuteBatchTransactionV2Args) -> Result<()> {
        Self::validate_signer(&self.settings, args.signer_key)?;
        Self::validate_proposal(&self.settings, &self.proposal)?;

        Ok(())
    }
}
