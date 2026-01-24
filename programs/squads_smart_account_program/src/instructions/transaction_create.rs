use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::interface::consensus_trait::ConsensusAccountType;
use crate::events::*;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;
use crate::utils::{create_transaction_message, verify_v2_context, *};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TransactionPayload {
    pub account_index: u8,
    pub ephemeral_signers: u8,
    pub transaction_message: Vec<u8>,
    pub memo: Option<String>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub enum CreateTransactionArgs {
    TransactionPayload(TransactionPayload),
    PolicyPayload {
        /// The payload of the policy transaction.
        payload: PolicyPayload,
    },
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateTransactionV2Args {
    pub create_args: CreateTransactionArgs,
    /// The key (Native) or key_id (External) of the creator
    pub creator_key: Pubkey,
    /// Client data params for WebAuthn verification (required for WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
}

#[derive(Accounts)]
#[instruction(args: CreateTransactionArgs)]
pub struct CreateTransaction<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    #[account(
        init,
        payer = rent_payer,
        space = match &args {
            CreateTransactionArgs::TransactionPayload(TransactionPayload { ephemeral_signers, transaction_message, .. }) => {
                Transaction::size_for_transaction(*ephemeral_signers, transaction_message)?
            },
            CreateTransactionArgs::PolicyPayload { payload } => {
                Transaction::size_for_policy(payload)?
            }
        },
        seeds = [
            SEED_PREFIX,
            consensus_account.key().as_ref(),
            SEED_TRANSACTION,
            &consensus_account.transaction_index().checked_add(1).unwrap().to_le_bytes(),
        ],
        bump
    )]
    pub transaction: Account<'info, Transaction>,

    /// The member of the multisig that is creating the transaction.
    pub creator: Signer<'info>,

    /// The payer for the transaction account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl<'info> CreateTransaction<'info> {
    pub fn validate(&self, ctx: &Context<Self>, args: &CreateTransactionArgs) -> Result<()> {
        validate_create_transaction(
            &self.consensus_account,
            &ctx.remaining_accounts,
            args,
            self.creator.key(),
        )
    }

    /// Create a new vault transaction.
    #[access_control(ctx.accounts.validate(&ctx, &args))]
    pub fn create_transaction(ctx: Context<Self>, args: CreateTransactionArgs) -> Result<()> {
        create_transaction_inner(
            &mut ctx.accounts.consensus_account,
            &mut ctx.accounts.transaction,
            &mut ctx.accounts.creator,
            &mut ctx.accounts.rent_payer,
            args,
            &ctx.accounts.program,
            *ctx.program_id,
        )
    }
}

#[derive(Accounts)]
#[instruction(args: CreateTransactionV2Args)]
pub struct CreateTransactionV2<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok()
    )]
    pub consensus_account: InterfaceAccount<'info, ConsensusAccount>,

    #[account(
        init,
        payer = rent_payer,
        space = match &args.create_args {
            CreateTransactionArgs::TransactionPayload(TransactionPayload { ephemeral_signers, transaction_message, .. }) => {
                Transaction::size_for_transaction(*ephemeral_signers, transaction_message)?
            },
            CreateTransactionArgs::PolicyPayload { payload } => {
                Transaction::size_for_policy(payload)?
            }
        },
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

impl<'info> CreateTransactionV2<'info> {
    pub fn validate(&self, ctx: &Context<Self>, args: &CreateTransactionV2Args) -> Result<()> {
        validate_create_transaction(
            &self.consensus_account,
            &ctx.remaining_accounts,
            &args.create_args,
            args.creator_key,
        )
    }

    /// Create a new vault transaction with V2 signer support.
    #[access_control(ctx.accounts.validate(&ctx, &args))]
    pub fn create_transaction_v2(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: CreateTransactionV2Args,
    ) -> Result<()> {
        let expected_message = create_transaction_message(
            &ctx.accounts.consensus_account.key(),
            ctx.accounts.consensus_account.transaction_index() + 1,
        );

        verify_v2_context(
            &mut ctx.accounts.consensus_account,
            args.creator_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        create_transaction_inner_v2(
            &mut ctx.accounts.consensus_account,
            &mut ctx.accounts.transaction,
            args.creator_key,
            &ctx.accounts.rent_payer,
            args.create_args,
            &ctx.accounts.program,
            *ctx.program_id,
        )
    }
}

pub(crate) fn validate_create_transaction(
    consensus_account: &InterfaceAccount<ConsensusAccount>,
    remaining_accounts: &[AccountInfo],
    args: &CreateTransactionArgs,
    creator_key: Pubkey,
) -> Result<()> {
    // Check if the consensus account is active
    consensus_account.is_active(remaining_accounts)?;

    // Validate the transaction payload
    match consensus_account.account_type() {
        ConsensusAccountType::Settings => match args {
            CreateTransactionArgs::TransactionPayload(TransactionPayload {
                account_index,
                ..
            }) => {
                let settings = consensus_account.read_only_settings()?;
                settings.validate_account_index_unlocked(*account_index)?;
            }
            _ => {
                return Err(SmartAccountError::InvalidTransactionMessage.into());
            }
        },
        ConsensusAccountType::Policy => {
            let policy = consensus_account.read_only_policy()?;
            match args {
                CreateTransactionArgs::PolicyPayload { payload } => {
                    policy.validate_payload(PolicyExecutionContext::Asynchronous, payload)?;
                }
                _ => {
                    return Err(SmartAccountError::InvalidTransactionMessage.into());
                }
            }
        }
    }

    // creator
    require!(
        consensus_account.is_signer(creator_key).is_some(),
        SmartAccountError::NotASigner
    );
    require!(
        consensus_account.signer_has_permission(creator_key, Permission::Initiate),
        SmartAccountError::Unauthorized
    );

    Ok(())
}

fn create_transaction_inner<'info>(
    consensus_account: &mut InterfaceAccount<'info, ConsensusAccount>,
    transaction: &mut Account<'info, Transaction>,
    creator: &mut Signer<'info>,
    rent_payer: &mut Signer<'info>,
    args: CreateTransactionArgs,
    program: &Program<'info, SquadsSmartAccountProgram>,
    program_id: Pubkey,
) -> Result<()> {
    create_transaction_inner_v2(
        consensus_account,
        transaction,
        creator.key(),
        rent_payer,
        args,
        program,
        program_id,
    )
}

pub(crate) fn create_transaction_inner_v2<'info>(
    consensus_account: &mut InterfaceAccount<'info, ConsensusAccount>,
    transaction: &mut Account<'info, Transaction>,
    creator_key: Pubkey,
    rent_payer: &Signer<'info>,
    args: CreateTransactionArgs,
    program: &Program<'info, SquadsSmartAccountProgram>,
    program_id: Pubkey,
) -> Result<()> {
    let transaction_key = transaction.key();

    let transaction_index = consensus_account
        .transaction_index()
        .checked_add(1)
        .unwrap();

    transaction.consensus_account = consensus_account.key();
    transaction.creator = creator_key;
    transaction.rent_collector = rent_payer.key();
    transaction.index = transaction_index;
    match (args, consensus_account.account_type()) {
        (
            CreateTransactionArgs::TransactionPayload(TransactionPayload {
                account_index,
                ephemeral_signers,
                transaction_message,
                memo: _,
            }),
            ConsensusAccountType::Settings,
        ) => {
            let transaction_message_parsed =
                TransactionMessage::deserialize(&mut transaction_message.as_slice())?;

            let ephemeral_signer_bumps: Vec<u8> = (0..ephemeral_signers)
                .map(|ephemeral_signer_index| {
                    let ephemeral_signer_seeds = &[
                        SEED_PREFIX,
                        transaction_key.as_ref(),
                        SEED_EPHEMERAL_SIGNER,
                        &ephemeral_signer_index.to_le_bytes(),
                    ];

                    let (_, bump) = Pubkey::find_program_address(ephemeral_signer_seeds, &program_id);
                    bump
                })
                .collect();

            transaction.payload = Payload::TransactionPayload(TransactionPayloadDetails {
                account_index: account_index,
                ephemeral_signer_bumps,
                message: transaction_message_parsed.try_into()?,
            });
        }
        (CreateTransactionArgs::PolicyPayload { payload }, ConsensusAccountType::Policy) => {
            transaction.payload = Payload::PolicyPayload(PolicyActionPayloadDetails { payload });
        }
        _ => {
            return Err(SmartAccountError::InvalidTransactionMessage.into());
        }
    }

    consensus_account.set_transaction_index(transaction_index)?;

    consensus_account.invariant()?;

    let event = TransactionEvent {
        event_type: TransactionEventType::Create,
        consensus_account: consensus_account.key(),
        consensus_account_type: consensus_account.account_type(),
        transaction_pubkey: transaction.key(),
        transaction_index,
        signer: Some(creator_key),
        transaction_content: Some(TransactionContent::Transaction(transaction.clone().into_inner())),
        memo: None,
    };

    let log_authority_info = LogAuthorityInfo {
        authority: consensus_account.to_account_info(),
        authority_seeds: consensus_account.get_signer_seeds(),
        bump: consensus_account.bump(),
        program: program.to_account_info(),
    };
    SmartAccountEvent::TransactionEvent(event).log(&log_authority_info)?;

    Ok(())
}

/// Unvalidated instruction data, must be treated as untrusted.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TransactionMessage {
    /// The number of signer pubkeys in the account_keys vec.
    pub num_signers: u8,
    /// The number of writable signer pubkeys in the account_keys vec.
    pub num_writable_signers: u8,
    /// The number of writable non-signer pubkeys in the account_keys vec.
    pub num_writable_non_signers: u8,
    /// The list of unique account public keys (including program IDs) that will be used in the provided instructions.
    pub account_keys: SmallVec<u8, Pubkey>,
    /// The list of instructions to execute.
    pub instructions: SmallVec<u8, CompiledInstruction>,
    /// List of address table lookups used to load additional accounts
    /// for this transaction.
    pub address_table_lookups: SmallVec<u8, MessageAddressTableLookup>,
}

// Concise serialization schema for instructions that make up transaction.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CompiledInstruction {
    pub program_id_index: u8,
    /// Indices into the tx's `account_keys` list indicating which accounts to pass to the instruction.
    pub account_indexes: SmallVec<u8, u8>,
    /// Instruction data.
    pub data: SmallVec<u16, u8>,
}

/// Address table lookups describe an on-chain address lookup table to use
/// for loading more readonly and writable accounts in a single tx.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct MessageAddressTableLookup {
    /// Address lookup table account key
    pub account_key: Pubkey,
    /// List of indexes used to load writable account addresses
    pub writable_indexes: SmallVec<u8, u8>,
    /// List of indexes used to load readonly account addresses
    pub readonly_indexes: SmallVec<u8, u8>,
}
