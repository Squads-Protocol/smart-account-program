use account_events::{AddSpendingLimitEvent, RemoveSpendingLimitEvent};
use anchor_lang::prelude::*;

use crate::{ consensus::ConsensusAccount, consensus_trait::{Consensus, ConsensusAccountType}, errors::*, events::*, program::SquadsSmartAccountProgram, state::*, utils::*};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SyncSettingsTransactionArgs {
    /// The number of signers to reach threshold and adequate permissions
    pub num_signers: u8,
    /// The settings actions to execute
    pub actions: Vec<SettingsAction>,
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct SyncSettingsTransaction<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok(),
        constraint = consensus_account.account_type() == ConsensusAccountType::Settings
    )]
    pub consensus_account: Box<InterfaceAccount<'info, ConsensusAccount>>,

    /// The account that will be charged/credited in case the settings transaction causes space reallocation,
    /// for example when adding a new signer, adding or removing a spending limit.
    /// This is usually the same as `signer`, but can be a different account if needed.
    #[account(mut)]
    pub rent_payer: Option<Signer<'info>>,

    /// We might need it in case reallocation is needed.
    pub system_program: Option<Program<'info, System>>,
    // `remaining_accounts` must include the following accounts in the exact order:
    // 1. The amount of signers specified in `num_signers`
    // 2. Any SpendingLimit accounts that need to be initialized/closed based on actions
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl<'info> SyncSettingsTransaction<'info> {
    fn validate(
        &self,
        args: &SyncSettingsTransactionArgs,
        remaining_accounts: &[AccountInfo],
    ) -> Result<()> {
        let Self { consensus_account, .. } = self;
        // Get the settings
        let settings = consensus_account.read_only_settings()?;

        // Settings must not be controlled
        require_keys_eq!(
            settings.settings_authority,
            Pubkey::default(),
            SmartAccountError::NotSupportedForControlled
        );

        // Validates the proposed settings changes
        validate_settings_actions(&args.actions)?;

        // Validates synchronous consensus across the signers
        validate_synchronous_consensus(&consensus_account, args.num_signers, remaining_accounts)?;

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts))]
    pub fn sync_settings_transaction(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncSettingsTransactionArgs,
    ) -> Result<()> {
        // Wrapper consensus account
        let consensus_account = &mut ctx.accounts.consensus_account;
        let settings_key = consensus_account.key();
        let settings_account_info = consensus_account.to_account_info();

        let settings = consensus_account.settings()?;

        let rent = Rent::get()?;

        // Build the log authority info
        let log_authority_info = LogAuthorityInfo {
            authority: settings_account_info.clone(),
            authority_seeds: get_settings_signer_seeds(settings.seed),
            bump: settings.bump,
            program: ctx.accounts.program.to_account_info(),
        };

        // Execute the actions one by one
        for action in args.actions.iter() {
            settings.modify_with_action(
                &settings_key,
                action,
                &rent,
                &ctx.accounts.rent_payer,
                &ctx.accounts.system_program,
                &ctx.remaining_accounts,
                &ctx.program_id,
                Some(&log_authority_info),
            )?;
        }

        // Make sure the smart account can fit the updated state: added signers or newly set archival_authority.
        Settings::realloc_if_needed(
            settings_account_info,
            settings.signers.len(),
            ctx.accounts
                .rent_payer
                .as_ref()
                .map(ToAccountInfo::to_account_info),
            ctx.accounts
                .system_program
                .as_ref()
                .map(ToAccountInfo::to_account_info),
        )?;

        // Make sure the settings state is valid after applying the actions
        settings.invariant()?;

        // Log the event
        let event = SynchronousSettingsTransactionEvent {
            settings_pubkey: settings_key,
            signers: ctx.remaining_accounts[..args.num_signers as usize]
                .iter()
                .map(|acc| acc.key.clone())
                .collect::<Vec<_>>(),
            settings: Settings::try_from_slice(&settings.try_to_vec()?)?,
            changes: args.actions.clone(),
        };

        SmartAccountEvent::SynchronousSettingsTransactionEvent(event).log(&log_authority_info)?;

        Ok(())
    }
}
