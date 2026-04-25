use crate::SmartAccountEventExt;
use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::error_conv::ToAnchorResult;
use crate::errors::*;
use crate::events::*;
use crate::interface::consensus::ConsensusAccount;
use crate::interface::consensus_trait::ConsensusAccountType;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;

// Re-export wire types and args from the types crate.
pub use squads_smart_account_program_types::{
    CompiledInstruction, CreateTransactionArgs, MessageAddressTableLookup, TransactionMessage,
    TransactionPayload,
};

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
                Transaction::size_for_transaction(*ephemeral_signers, transaction_message).map_err(crate::error_conv::types_err_to_anchor)?
            },
            CreateTransactionArgs::PolicyPayload { payload } => {
                Transaction::size_for_policy(payload).map_err(crate::error_conv::types_err_to_anchor)?
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
        let Self {
            consensus_account,
            creator,
            ..
        } = self;

        // Check if the consensus account is active
        consensus_account.is_active(ctx.remaining_accounts)?;

        // Validate the transaction payload
        match consensus_account.account_type() {
            ConsensusAccountType::Settings => {
                assert!(matches!(
                    args,
                    CreateTransactionArgs::TransactionPayload { .. }
                ));
            }
            ConsensusAccountType::Policy => {
                let policy = consensus_account.read_only_policy()?;
                // Validate that the args match the policy type
                match args {
                    CreateTransactionArgs::PolicyPayload { payload } => {
                        // Validate the policy payload against the policy state
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
            consensus_account.is_signer(creator.key()).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            consensus_account.signer_has_permission(creator.key(), Permission::Initiate),
            SmartAccountError::Unauthorized
        );

        Ok(())
    }

    /// Create a new vault transaction.
    #[access_control(ctx.accounts.validate(&ctx, &args))]
    pub fn create_transaction(ctx: Context<Self>, args: CreateTransactionArgs) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let transaction = &mut ctx.accounts.transaction;
        let creator = &mut ctx.accounts.creator;
        let rent_payer = &mut ctx.accounts.rent_payer;

        let transaction_key = transaction.key();

        // Increment the transaction index.
        let transaction_index = consensus_account
            .transaction_index()
            .checked_add(1)
            .unwrap();

        // Initialize the transaction fields.
        transaction.consensus_account = consensus_account.key();
        transaction.creator = creator.key();
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

                        let (_, bump) =
                            Pubkey::find_program_address(ephemeral_signer_seeds, ctx.program_id);
                        bump
                    })
                    .collect();

                transaction.payload = Payload::TransactionPayload(TransactionPayloadDetails {
                    account_index,
                    ephemeral_signer_bumps,
                    message: transaction_message_parsed.try_into().to_anchor()?,
                });
            }
            (CreateTransactionArgs::PolicyPayload { payload }, ConsensusAccountType::Policy) => {
                transaction.payload =
                    Payload::PolicyPayload(PolicyActionPayloadDetails { payload });
            }
            _ => {
                return Err(SmartAccountError::InvalidTransactionMessage.into());
            }
        }

        // Updated last transaction index in the settings account.
        consensus_account.set_transaction_index(transaction_index)?;

        consensus_account.invariant()?;

        // Transaction event
        let event = TransactionEvent {
            event_type: TransactionEventType::Create,
            consensus_account: consensus_account.key(),
            consensus_account_type: consensus_account.account_type(),
            transaction_pubkey: transaction.key(),
            transaction_index,
            signer: Some(creator.key()),
            transaction_content: Some(TransactionContent::Transaction(
                transaction.clone().into_inner(),
            )),
            memo: None,
        };

        // Log event authority info
        let log_authority_info = LogAuthorityInfo {
            authority: consensus_account.to_account_info(),
            authority_seeds: consensus_account.get_signer_seeds(),
            bump: consensus_account.bump(),
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::TransactionEvent(event).log(&log_authority_info)?;

        Ok(())
    }
}
