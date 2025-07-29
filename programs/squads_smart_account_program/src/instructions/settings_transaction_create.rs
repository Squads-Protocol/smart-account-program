use anchor_lang::prelude::*;

use crate::consensus_trait::{Consensus, ConsensusAccountType};
use crate::program::SquadsSmartAccountProgram;
use crate::{state::*, SmartAccountEvent};
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
        // settings
        require_keys_eq!(
            self.settings.settings_authority,
            Pubkey::default(),
            SmartAccountError::NotSupportedForControlled
        );

        // creator
        require!(
            self.settings.is_signer(self.creator.key()).is_some(),
            SmartAccountError::NotASigner
        );
        require!(
            self.settings
                .signer_has_permission(self.creator.key(), Permission::Initiate),
            SmartAccountError::Unauthorized
        );

        // args
        validate_settings_actions(&args.actions)?;

        Ok(())
    }

    /// Create a new settings transaction.
    #[access_control(ctx.accounts.validate(&args))]
    pub fn create_settings_transaction(
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

        // Initialize the transaction fields.
        transaction.settings = settings_key;
        transaction.creator = creator.key();
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
            signer: Some(creator.key()),
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
