use crate::consensus_trait::{Consensus, ConsensusAccountType};
use crate::errors::*;
use crate::instructions::*;
use crate::state::*;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::create_transaction_from_buffer_message;
use anchor_lang::{prelude::*, system_program};

#[derive(Accounts)]
pub struct CreateTransactionFromBuffer<'info> {
    // The context needed for the CreateTransaction instruction
    pub transaction_create: CreateTransaction<'info>,

    #[account(
        mut,
        close = rent_collector,
        has_one = rent_collector,
        // PDA derived from stored creator (canonical key for V2, raw key for V1)
        seeds = [
            SEED_PREFIX,
            transaction_create.consensus_account.key().as_ref(),
            SEED_TRANSACTION_BUFFER,
            transaction_buffer.creator.as_ref(),
            &transaction_buffer.buffer_index.to_le_bytes(),
        ],
        bump
    )]
    pub transaction_buffer: Box<Account<'info, TransactionBuffer>>,

    // Anchor doesn't allow us to use the creator inside of
    // transaction_create, so we just re-pass it here with the same constraint
    /// CHECK: Must match transaction_create.creator
    #[account(
        address = transaction_create.creator.key(),
    )]
    pub creator: AccountInfo<'info>,

    /// CHECK: Validated via has_one on transaction_buffer.
    /// Receives lamports on buffer close.
    #[account(mut)]
    pub rent_collector: AccountInfo<'info>,
}

impl<'info> CreateTransactionFromBuffer<'info> {
    pub fn validate(
        &mut self,
        args: &CreateTransactionArgs,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        let transaction_buffer_account = &self.transaction_buffer;
        let consensus_account = &mut self.transaction_create.consensus_account;
        let creator = &self.creator;

        // Build message for external signer verification
        let message = create_transaction_from_buffer_message(
            &transaction_buffer_account.key(),
            consensus_account.transaction_index().checked_add(1).unwrap(),
        );

        // Strip instructions sysvar (if at [0]) before is_active so
        // SettingsState expiration sees the Settings account at [0].
        let accounts_for_active = if remaining_accounts
            .first()
            .map_or(false, |acc| acc.key == &anchor_lang::solana_program::sysvar::instructions::ID)
        { &remaining_accounts[1..] } else { remaining_accounts };

        // Check if the consensus account is active
        consensus_account.is_active(accounts_for_active)?;

        // Validate account index is unlocked for Settings-based transactions
        if consensus_account.account_type() == ConsensusAccountType::Settings {
            match args {
                CreateTransactionArgs::TransactionPayload(TransactionPayload { account_index, .. }) => {
                    let settings = consensus_account.read_only_settings()?;
                    settings.validate_account_index_unlocked(*account_index)?;
                }
                _ => {}
            }
        }

        // Resolve and verify signer (native, session key, or external) and check Initiate permission
        self.transaction_create.creator.verify(
            &mut **consensus_account,
            remaining_accounts,
            message,
            extra_verification_data.as_ref(),
            Some(Permission::Initiate),
        )?;

        // Check that the transaction message is "empty" and this is a TransactionPayload
        match args {
            CreateTransactionArgs::PolicyPayload { .. } => {
                return Err(SmartAccountError::InvalidInstructionArgs.into())
            }
            CreateTransactionArgs::TransactionPayload(TransactionPayload {
                transaction_message,
                ..
            }) => {
                require!(
                    transaction_message == &vec![0, 0, 0, 0, 0, 0],
                    SmartAccountError::InvalidInstructionArgs
                );
            }
        }

        // Validate that the final hash matches the buffer
        transaction_buffer_account.validate_hash()?;

        // Validate that the final size is correct
        transaction_buffer_account.validate_size()?;

        Ok(())
    }
    /// Create a new Transaction from a completed transaction buffer account.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, None))]
    pub fn create_transaction_from_buffer(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: CreateTransactionArgs,
    ) -> Result<()> {
        // V1: raw key must match stored creator
        require!(
            ctx.accounts.transaction_buffer.creator == ctx.accounts.creator.key(),
            SmartAccountError::Unauthorized
        );
        Self::create_transaction_from_buffer_inner(ctx, args)
    }

    /// Create a new Transaction from a completed transaction buffer account with V2 signer support.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, extra_verification_data))]
    pub fn create_transaction_from_buffer_v2(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: CreateTransactionArgs,
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        // V2: resolved key must match stored creator (session keys resolve to parent)
        let resolved_key = ctx.accounts.transaction_create.creator.resolved_key()?;
        require!(
            ctx.accounts.transaction_buffer.creator == resolved_key,
            SmartAccountError::Unauthorized
        );
        Self::create_transaction_from_buffer_inner(ctx, args)
    }

    fn create_transaction_from_buffer_inner(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: CreateTransactionArgs,
    ) -> Result<()> {
        // Account infos necessary for reallocation
        let transaction_account_info = &ctx
            .accounts
            .transaction_create
            .transaction
            .to_account_info();
        let rent_payer_account_info = &ctx.accounts.transaction_create.rent_payer.to_account_info();

        let system_program = &ctx
            .accounts
            .transaction_create
            .system_program
            .to_account_info();

        // Read-only accounts
        let transaction_buffer = &ctx.accounts.transaction_buffer;

        // Calculate the new required length of the transaction account,
        // since it was initialized with an empty transaction message
        let new_len = match &args {
            CreateTransactionArgs::TransactionPayload(TransactionPayload {
                ephemeral_signers,
                ..
            }) => {
                Transaction::size_for_transaction(*ephemeral_signers, &transaction_buffer.buffer)?
            }
            CreateTransactionArgs::PolicyPayload { .. } => {
                return Err(SmartAccountError::InvalidInstructionArgs.into())
            }
        };

        // Calculate the rent exemption for new length
        let rent_exempt_lamports = Rent::get().unwrap().minimum_balance(new_len).max(1);

        // Check the difference between the rent exemption and the current lamports
        let top_up_lamports =
            rent_exempt_lamports.saturating_sub(transaction_account_info.lamports());

        // System Transfer the remaining difference to the transaction account
        let transfer_context = CpiContext::new(
            system_program.to_account_info(),
            system_program::Transfer {
                from: rent_payer_account_info.clone(),
                to: transaction_account_info.clone(),
            },
        );
        system_program::transfer(transfer_context, top_up_lamports)?;

        // Reallocate the transaction account to the new length of the
        // actual transaction message
        AccountInfo::realloc(&transaction_account_info, new_len, true)?;

        // Create the args for the `create_transaction` instruction
        let create_args = match &args {
            CreateTransactionArgs::TransactionPayload(TransactionPayload {
                account_index,
                ephemeral_signers,
                memo,
                ..
            }) => CreateTransactionArgs::TransactionPayload(TransactionPayload {
                account_index: *account_index,
                ephemeral_signers: *ephemeral_signers,
                transaction_message: transaction_buffer.buffer.clone(),
                memo: memo.clone(),
            }),
            CreateTransactionArgs::PolicyPayload { .. } => {
                return Err(SmartAccountError::InvalidInstructionArgs.into())
            }
        };
        // Create the context for the `create_transaction` instruction
        let context = Context::new(
            ctx.program_id,
            &mut ctx.accounts.transaction_create,
            ctx.remaining_accounts,
            ctx.bumps.transaction_create,
        );

        // Call create_transaction_inner directly — bypasses double verify_signer.
        // Validation (is_active, account_index_unlocked, verify_signer) already ran above.
        CreateTransaction::create_transaction_inner(context, create_args)?;

        Ok(())
    }
}
