use anchor_lang::prelude::*;

use crate::consensus_trait::{Consensus, ConsensusAccountType};
use crate::program::SquadsSmartAccountProgram;
use crate::{state::*, SmartAccountEvent};
use crate::utils::{create_settings_transaction_create_message, validate_settings_actions, verify_v2_context};
use crate::LogAuthorityInfo;
use crate::{errors::*, TransactionContent, TransactionEvent, TransactionEventType};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateSettingsTransactionArgs {
    pub actions: Vec<SettingsAction>,
    pub memo: Option<String>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateSettingsTransactionV2Args {
    pub actions: Vec<SettingsAction>,
    /// The key (Native) or key_id (External) of the creator
    pub creator_key: Pubkey,
    /// Client data params for WebAuthn verification (required for WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
    pub memo: Option<String>,
}

#[derive(Accounts)]
#[instruction(args: CreateSettingsTransactionArgs)]
pub struct CreateSettingsTransaction<'info> {
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    #[account(
        init,
        payer = rent_payer,
        space = SettingsTransaction::size(&args.actions),
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &settings.transaction_index.checked_add(1).unwrap().to_le_bytes(),
        ],
        bump
    )]
    pub transaction: Account<'info, SettingsTransaction>,

    /// The signer on the smart account that is creating the transaction.
    pub creator: Signer<'info>,

    /// The payer for the transaction account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl CreateSettingsTransaction<'_> {
    fn validate(&self, args: &CreateSettingsTransactionArgs) -> Result<()> {
        validate_create_settings_transaction(&self.settings, self.creator.key(), &args.actions)
    }

    /// Create a new settings transaction.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn create_settings_transaction(
        ctx: Context<Self>,
        args: CreateSettingsTransactionArgs,
    ) -> Result<()> {
        create_settings_transaction_inner(
            &mut ctx.accounts.settings,
            &mut ctx.accounts.transaction,
            ctx.accounts.creator.key(),
            &ctx.accounts.rent_payer,
            ctx.bumps.transaction,
            args.actions,
            &ctx.accounts.program,
        )
    }
}

#[derive(Accounts)]
#[instruction(args: CreateSettingsTransactionV2Args)]
pub struct CreateSettingsTransactionV2<'info> {
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.to_le_bytes().as_ref()],
        bump = settings.bump,
    )]
    pub settings: Account<'info, Settings>,

    #[account(
        init,
        payer = rent_payer,
        space = SettingsTransaction::size(&args.actions),
        seeds = [
            SEED_PREFIX,
            settings.key().as_ref(),
            SEED_TRANSACTION,
            &settings.transaction_index.checked_add(1).unwrap().to_le_bytes(),
        ],
        bump
    )]
    pub transaction: Account<'info, SettingsTransaction>,

    /// The payer for the transaction account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl CreateSettingsTransactionV2<'_> {
    fn validate(&self, args: &CreateSettingsTransactionV2Args) -> Result<()> {
        validate_create_settings_transaction(&self.settings, args.creator_key, &args.actions)
    }

    /// Create a new settings transaction with V2 signer support.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn create_settings_transaction_v2(
        ctx: Context<Self>,
        args: CreateSettingsTransactionV2Args,
    ) -> Result<()> {
        let expected_message = create_settings_transaction_create_message(
            &ctx.accounts.settings.key(),
            ctx.accounts.settings.transaction_index + 1,
        );

        verify_v2_context(
            &mut *ctx.accounts.settings,
            args.creator_key,
            &ctx.remaining_accounts,
            &expected_message,
            args.client_data_params.as_ref(),
        )?;

        create_settings_transaction_inner(
            &mut ctx.accounts.settings,
            &mut ctx.accounts.transaction,
            args.creator_key,
            &ctx.accounts.rent_payer,
            ctx.bumps.transaction,
            args.actions,
            &ctx.accounts.program,
        )
    }
}

fn validate_create_settings_transaction(
    settings: &Settings,
    creator_key: Pubkey,
    actions: &[SettingsAction],
) -> Result<()> {
    // Settings must not be controlled
    require_keys_eq!(
        settings.settings_authority,
        Pubkey::default(),
        SmartAccountError::NotSupportedForControlled
    );

    // Creator must be a signer on the smart account
    require!(
        settings.is_signer(creator_key).is_some(),
        SmartAccountError::NotASigner
    );
    require!(
        settings.signer_has_permission(creator_key, Permission::Initiate),
        SmartAccountError::Unauthorized
    );

    // Validate actions
    validate_settings_actions(actions)?;

    Ok(())
}

fn create_settings_transaction_inner<'info>(
    settings: &mut Account<'info, Settings>,
    transaction: &mut Account<'info, SettingsTransaction>,
    creator_key: Pubkey,
    rent_payer: &Signer<'info>,
    transaction_bump: u8,
    actions: Vec<SettingsAction>,
    program: &Program<'info, SquadsSmartAccountProgram>,
) -> Result<()> {
    let settings_key = settings.key();

    // Increment the transaction index.
    let transaction_index = settings.transaction_index.checked_add(1).unwrap();

    // Initialize the transaction fields.
    transaction.settings = settings_key;
    transaction.creator = creator_key;
    transaction.rent_collector = rent_payer.key();
    transaction.index = transaction_index;
    transaction.bump = transaction_bump;
    transaction.actions = actions.clone();

    // Updated last transaction index in the settings account.
    settings.transaction_index = transaction_index;

    settings.invariant()?;

    // Log event authority info
    let log_authority_info = LogAuthorityInfo {
        authority: settings.to_account_info().clone(),
        authority_seeds: get_settings_signer_seeds(settings.seed),
        bump: settings.bump,
        program: program.to_account_info(),
    };

    // Log the event
    let event = TransactionEvent {
        event_type: TransactionEventType::Create,
        consensus_account: settings.key(),
        consensus_account_type: ConsensusAccountType::Settings,
        transaction_pubkey: transaction.key(),
        transaction_index,
        signer: Some(creator_key),
        transaction_content: Some(TransactionContent::SettingsTransaction {
            settings: settings.clone().into_inner(),
            transaction: transaction.clone().into_inner(),
            changes: actions,
        }),
        memo: None,
    };
    SmartAccountEvent::TransactionEvent(event).log(&log_authority_info)?;
    Ok(())
}
