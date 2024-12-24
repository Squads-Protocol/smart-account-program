use anchor_lang::prelude::*;

use crate::errors::*;
use crate::state::*;
use crate::utils::validate_settings_actions;

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
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.as_ref()],
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

        let settings_key = settings.key();

        // Increment the transaction index.
        let transaction_index = settings.transaction_index.checked_add(1).unwrap();

        // Initialize the transaction fields.
        transaction.settings = settings_key;
        transaction.creator = creator.key();
        transaction.index = transaction_index;
        transaction.bump = ctx.bumps.transaction;
        transaction.actions = args.actions;

        // Updated last transaction index in the settings account.
        settings.transaction_index = transaction_index;

        settings.invariant()?;

        // Logs for indexing.
        msg!("transaction index: {}", transaction_index);

        Ok(())
    }
}
