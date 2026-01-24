use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::instructions::{create_transaction_inner_v2, validate_create_transaction, *};
use crate::interface::consensus::ConsensusAccount;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;
use crate::utils::{create_transaction_from_buffer_message, verify_v2_context};
use anchor_lang::{prelude::*, system_program};

#[derive(Accounts)]
pub struct CreateTransactionFromBuffer<'info> {
    // The context needed for the CreateTransaction instruction
    pub transaction_create: CreateTransaction<'info>,

    #[account(
        mut,
        close = creator,
        // Only the creator can turn the buffer into a transaction and reclaim
        // the rent
        constraint = transaction_buffer.creator == creator.key() @ SmartAccountError::Unauthorized,
        seeds = [
            SEED_PREFIX,
            transaction_create.consensus_account.key().as_ref(),
            SEED_TRANSACTION_BUFFER,
            creator.key().as_ref(),
            &transaction_buffer.buffer_index.to_le_bytes(),
        ],
        bump
    )]
    pub transaction_buffer: Box<Account<'info, TransactionBuffer>>,

    // Anchor doesn't allow us to use the creator inside of
    // transaction_create, so we just re-pass it here with the same constraint
    #[account(
        mut,
        address = transaction_create.creator.key(),
    )]
    pub creator: Signer<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateTransactionFromBufferV2Args {
    /// Transaction creation args
    pub create_args: CreateTransactionArgs,
    /// The key (Native) or key_id (External) of the creator
    pub creator_key: Pubkey,
    /// Client data params for WebAuthn verification (required for WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
}

impl<'info> CreateTransactionFromBuffer<'info> {
    pub fn validate(&self, args: &CreateTransactionArgs) -> Result<()> {
        let transaction_buffer_account = &self.transaction_buffer;

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
    #[access_control(ctx.accounts.validate(&args))]
    pub fn create_transaction_from_buffer(
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

        // Call the `create_transaction` instruction
        CreateTransaction::create_transaction(context, create_args)?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(args: CreateTransactionFromBufferV2Args)]
pub struct CreateTransactionFromBufferV2<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    #[account(
        mut,
        close = rent_payer,
        constraint = transaction_buffer.creator == args.creator_key @ SmartAccountError::Unauthorized,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION_BUFFER,
            args.creator_key.as_ref(),
            &transaction_buffer.buffer_index.to_le_bytes(),
        ],
        bump
    )]
    pub transaction_buffer: Box<Account<'info, TransactionBuffer>>,

    /// The transaction to create.
    #[account(
        mut,
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION,
            &consensus_account.transaction_index().checked_add(1).unwrap().to_le_bytes(),
        ],
        bump
    )]
    pub transaction: Account<'info, Transaction>,

    /// The payer for the transaction account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl<'info> CreateTransactionFromBufferV2<'info> {
    pub fn validate(&self, args: &CreateTransactionArgs) -> Result<()> {
        let transaction_buffer_account = &self.transaction_buffer;

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

        transaction_buffer_account.validate_hash()?;
        transaction_buffer_account.validate_size()?;
        Ok(())
    }

    /// Create a new Transaction from a completed transaction buffer account with V2 signer support.
    #[access_control(ctx.accounts.validate(&args.create_args))]
    pub fn create_transaction_from_buffer_v2(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: CreateTransactionFromBufferV2Args,
    ) -> Result<()> {
        let expected_message = create_transaction_from_buffer_message(
            &ctx.accounts.transaction_buffer.key(),
            ctx.accounts.consensus_account.transaction_index() + 1,
        );

        verify_v2_context(
            &mut ctx.accounts.consensus_account,
            args.creator_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        let transaction_account_info = &ctx.accounts.transaction.to_account_info();
        let rent_payer_account_info = &ctx.accounts.rent_payer.to_account_info();
        let system_program = &ctx.accounts.system_program.to_account_info();
        let transaction_buffer = &ctx.accounts.transaction_buffer;

        let new_len = match &args.create_args {
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

        let rent_exempt_lamports = Rent::get().unwrap().minimum_balance(new_len).max(1);
        let top_up_lamports =
            rent_exempt_lamports.saturating_sub(transaction_account_info.lamports());

        let transfer_context = CpiContext::new(
            system_program.to_account_info(),
            system_program::Transfer {
                from: rent_payer_account_info.clone(),
                to: transaction_account_info.clone(),
            },
        );
        system_program::transfer(transfer_context, top_up_lamports)?;

        AccountInfo::realloc(&transaction_account_info, new_len, true)?;

        let create_args = match &args.create_args {
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

        validate_create_transaction(
            &ctx.accounts.consensus_account,
            &ctx.remaining_accounts,
            &create_args,
            args.creator_key,
        )?;

        create_transaction_inner_v2(
            &mut ctx.accounts.consensus_account,
            &mut ctx.accounts.transaction,
            args.creator_key,
            &ctx.accounts.rent_payer,
            create_args,
            &ctx.accounts.program,
            *ctx.program_id,
        )
    }
}
