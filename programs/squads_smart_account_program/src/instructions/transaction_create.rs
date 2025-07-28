use anchor_lang::prelude::*;

use crate::consensus_trait::Consensus;
use crate::errors::*;
use crate::interface::consensus::ConsensusAccount;
use crate::interface::consensus_trait::ConsensusAccountType;
use crate::events::*;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;
use crate::utils::*;

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
        let Self {
            consensus_account,
            creator,
            ..
        } = self;

        // Check if the consensus account is active
        consensus_account.is_active(&ctx.remaining_accounts)?;

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
                    account_index: account_index,
                    ephemeral_signer_bumps,
                    message: transaction_message_parsed.try_into()?,
                });
            }
            (CreateTransactionArgs::PolicyPayload { payload }, ConsensusAccountType::Policy) => {
                transaction.payload =
                    Payload::PolicyPayload(PolicyActionPayloadDetails { payload: payload });
            }
            _ => {
                return Err(SmartAccountError::InvalidTransactionMessage.into());
            }
        }

        // Updated last transaction index in the settings account.
        consensus_account.set_transaction_index(transaction_index)?;

        consensus_account.invariant()?;

        // Log the event
        let event = TransactionEvent {
            event_type: TransactionEventType::Create,
            settings_pubkey: settings.key(),
            transaction_pubkey: transaction.key(),
            transaction_index,
            signer: Some(creator.key()),
            transaction: Some(Transaction::try_from_slice(&transaction.try_to_vec()?)?),
            memo: args.memo,
        };
        let log_authority_info = LogAuthorityInfo {
            authority: settings.to_account_info(),
            authority_seeds: get_settings_signer_seeds(settings.seed),
            bump: settings.bump,
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::TransactionEvent(event).log(&log_authority_info)?;

        Ok(())
    }
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
