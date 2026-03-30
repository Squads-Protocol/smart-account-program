use anchor_lang::prelude::*;
use anchor_lang::system_program;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::state::MAX_BUFFER_SIZE;
use crate::state::*;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::create_transaction_buffer_create_message;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateTransactionBufferArgs {
    /// Index of the buffer account to seed the account derivation
    pub buffer_index: u8,
    /// Index of the smart account this transaction belongs to.
    pub account_index: u8,
    /// Hash of the final assembled transaction message.
    pub final_buffer_hash: [u8; 32],
    /// Final size of the buffer.
    pub final_buffer_size: u16,
    /// Initial slice of the buffer.
    pub buffer: Vec<u8>,
}

#[derive(Accounts)]
#[instruction(args: CreateTransactionBufferArgs)]
pub struct CreateTransactionBuffer<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    /// CHECK: Initialized manually in handler body. PDA verified against derived address.
    #[account(mut)]
    pub transaction_buffer: AccountInfo<'info>,

    /// CHECK: Verified via verify_signer (native, session key, or external)
    pub creator: AccountInfo<'info>,

    /// The payer for the transaction account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> CreateTransactionBuffer<'info> {
    fn validate(
        &mut self,
        args: &CreateTransactionBufferArgs,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        let Self {
            consensus_account, creator, ..
        } = self;

        // Build message for external signer verification
        let message = create_transaction_buffer_create_message(
            &consensus_account.key(),
            creator.key(),
            args.buffer_index,
            args.account_index,
            &args.final_buffer_hash,
            args.final_buffer_size,
        );

        // Verify signer (native, session key, or external) and check Initiate permission
        consensus_account.verify_signer(
            creator,
            remaining_accounts,
            message,
            extra_verification_data.as_ref(),
            Some(Permission::Initiate),
        )?;

        // Final Buffer Size must not exceed 4000 bytes
        require!(
            args.final_buffer_size as usize <= MAX_BUFFER_SIZE,
            SmartAccountError::FinalBufferSizeExceeded
        );

        Ok(())
    }

    /// Create a new transaction buffer.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, None))]
    pub fn create_transaction_buffer(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: CreateTransactionBufferArgs,
    ) -> Result<()> {
        Self::create_transaction_buffer_inner(ctx, args, None)
    }

    /// Create a new transaction buffer with V2 signer support.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, extra_verification_data))]
    pub fn create_transaction_buffer_v2(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: CreateTransactionBufferArgs,
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        // V2: resolve canonical key for PDA derivation
        let canonical_key = ctx.accounts.consensus_account.resolve_canonical_key(
            ctx.accounts.creator.key(),
            ctx.accounts.creator.is_signer,
        )?;
        Self::create_transaction_buffer_inner(ctx, args, Some(canonical_key))
    }

    /// Shared inner: creates the buffer account at the correct PDA and populates it.
    /// `creator_override`: None = use raw creator.key() (V1), Some = use canonical key (V2).
    fn create_transaction_buffer_inner(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: CreateTransactionBufferArgs,
        creator_override: Option<Pubkey>,
    ) -> Result<()> {
        let consensus_account_key = ctx.accounts.consensus_account.key();
        let pda_creator_key = creator_override.unwrap_or(ctx.accounts.creator.key());

        // Derive PDA and verify the passed account matches
        let seeds: &[&[u8]] = &[
            SEED_PREFIX,
            consensus_account_key.as_ref(),
            SEED_TRANSACTION_BUFFER,
            pda_creator_key.as_ref(),
            &args.buffer_index.to_le_bytes(),
        ];
        let (expected_pda, bump) = Pubkey::find_program_address(seeds, ctx.program_id);
        require!(
            ctx.accounts.transaction_buffer.key() == expected_pda,
            SmartAccountError::InvalidAccount
        );

        // Create account at PDA (anti-DDoS: handle pre-funded accounts)
        let space = TransactionBuffer::size(args.final_buffer_size)?;
        let lamports = Rent::get()?.minimum_balance(space);
        let signer_seeds: &[&[u8]] = &[
            SEED_PREFIX,
            consensus_account_key.as_ref(),
            SEED_TRANSACTION_BUFFER,
            pda_creator_key.as_ref(),
            &args.buffer_index.to_le_bytes(),
            &[bump],
        ];
        let buffer_info = ctx.accounts.transaction_buffer.to_account_info();
        let payer_info = ctx.accounts.rent_payer.to_account_info();
        let system_info = ctx.accounts.system_program.to_account_info();

        if buffer_info.lamports() == 0 {
            // Normal path: create account from scratch
            system_program::create_account(
                CpiContext::new_with_signer(
                    system_info,
                    system_program::CreateAccount {
                        from: payer_info,
                        to: buffer_info,
                    },
                    &[signer_seeds],
                ),
                lamports,
                space as u64,
                ctx.program_id,
            )?;
        } else {
            // Anti-DDoS: account was pre-funded. Top up, allocate, assign.
            let required = lamports.saturating_sub(buffer_info.lamports());
            if required > 0 {
                system_program::transfer(
                    CpiContext::new(
                        system_info.clone(),
                        system_program::Transfer {
                            from: payer_info,
                            to: buffer_info.clone(),
                        },
                    ),
                    required,
                )?;
            }
            system_program::allocate(
                CpiContext::new_with_signer(
                    system_info.clone(),
                    system_program::Allocate {
                        account_to_allocate: buffer_info.clone(),
                    },
                    &[signer_seeds],
                ),
                space as u64,
            )?;
            system_program::assign(
                CpiContext::new_with_signer(
                    system_info,
                    system_program::Assign {
                        account_to_assign: buffer_info,
                    },
                    &[signer_seeds],
                ),
                ctx.program_id,
            )?;
        }

        // Initialize: write discriminator + data
        let buffer = TransactionBuffer {
            settings: consensus_account_key,
            creator: pda_creator_key,
            buffer_index: args.buffer_index,
            account_index: args.account_index,
            final_buffer_hash: args.final_buffer_hash,
            final_buffer_size: args.final_buffer_size,
            buffer: args.buffer,
        };
        buffer.invariant()?;

        let mut data = ctx.accounts.transaction_buffer.try_borrow_mut_data()?;
        let mut writer: &mut [u8] = &mut data;
        buffer.try_serialize(&mut writer)?;

        Ok(())
    }
}
