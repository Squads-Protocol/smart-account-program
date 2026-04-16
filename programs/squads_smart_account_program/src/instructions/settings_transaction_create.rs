use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash;

use crate::consensus_trait::ConsensusAccountType;
use crate::program::SquadsSmartAccountProgram;
use crate::{state::*, SmartAccountEvent};
use crate::instructions::*;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::create_settings_transaction_create_message;
use crate::utils::validate_settings_actions;
use crate::LogAuthorityInfo;
use crate::{errors::*, TransactionContent, TransactionEvent, TransactionEventType};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateSettingsTransactionArgs {
    pub actions: Vec<SettingsAction>,
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

    pub creator: ResolvedSigner<'info>,

    /// The payer for the transaction account rent.
    #[account(mut)]
    pub rent_payer: Signer<'info>,

    pub system_program: Program<'info, System>,

    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl CreateSettingsTransaction<'_> {
    fn validate(
        &mut self,
        args: &CreateSettingsTransactionArgs,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        let Self {
            settings,
            creator,
            ..
        } = self;

        // settings
        require_keys_eq!(
            settings.settings_authority,
            Pubkey::default(),
            SmartAccountError::NotSupportedForControlled
        );

        // Build message for external signer verification.
        // Hash the actions so external signers commit to the exact settings changes.
        let actions_bytes = args.actions.try_to_vec()
            .map_err(|_| SmartAccountError::InvalidPayload)?;
        let payload_hash = hash(&actions_bytes);
        let next_transaction_index = settings.transaction_index.checked_add(1).unwrap();
        let message = create_settings_transaction_create_message(
            &settings.key(),
            next_transaction_index,
            &payload_hash.to_bytes(),
        );

        // Resolve and verify signer (native, session key, or external) and check Initiate permission
        creator.verify(
            &mut **settings,
            remaining_accounts,
            message,
            extra_verification_data.as_ref(),
            Some(Permission::Initiate),
        )?;

        // args
        validate_settings_actions(&args.actions)?;

        Ok(())
    }

    /// Create a new settings transaction.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, None))]
    pub fn create_settings_transaction(
        ctx: Context<Self>,
        args: CreateSettingsTransactionArgs,
    ) -> Result<()> {
        Self::create_settings_transaction_inner(ctx, args)
    }

    /// Create a new settings transaction with V2 signer support.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, extra_verification_data))]
    pub fn create_settings_transaction_v2(
        ctx: Context<Self>,
        args: CreateSettingsTransactionArgs,
        extra_verification_data: Option<ExtraVerificationData>,
    ) -> Result<()> {
        Self::create_settings_transaction_inner(ctx, args)
    }

    fn create_settings_transaction_inner(
        ctx: Context<Self>,
        args: CreateSettingsTransactionArgs,
    ) -> Result<()> {
        let settings = &mut ctx.accounts.settings;
        let transaction = &mut ctx.accounts.transaction;
        let creator = &mut ctx.accounts.creator;
        let rent_payer = &mut ctx.accounts.rent_payer;
        let settings_key = settings.key();

        // Increment the transaction index.
        let transaction_index = settings.transaction_index.checked_add(1).unwrap();

        // Use resolved key for storage and events
        let resolved_key = creator.resolved_key()?;

        // Initialize the transaction fields.
        transaction.settings = settings_key;
        transaction.creator = resolved_key;
        transaction.rent_collector = rent_payer.key();
        transaction.index = transaction_index;
        transaction.bump = ctx.bumps.transaction;
        transaction.actions = args.actions.clone();

        // Updated last transaction index in the settings account.
        settings.transaction_index = transaction_index;

        settings.invariant()?;

        // Log event authority info
        let log_authority_info = LogAuthorityInfo {
            authority: settings.to_account_info().clone(),
            authority_seeds: get_settings_signer_seeds(settings.seed),
            bump: settings.bump,
            program: ctx.accounts.program.to_account_info(),
        };

        // Log the event
        let event = TransactionEvent {
            event_type: TransactionEventType::Create,
            consensus_account: settings.key(),
            consensus_account_type: ConsensusAccountType::Settings,
            transaction_pubkey: transaction.key(),
            transaction_index,
            signer: Some(resolved_key),
            transaction_content: Some(TransactionContent::SettingsTransaction {
                settings: settings.clone().into_inner(),
                transaction: transaction.clone().into_inner(),
                changes: args.actions,
            }),
            memo: None,
        };
        SmartAccountEvent::TransactionEvent(event).log(&log_authority_info)?;
        Ok(())
    }
}
