use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash;

use crate::{consensus::ConsensusAccount, consensus_trait::{Consensus, ConsensusAccountType}, errors::*, events::*, program::SquadsSmartAccountProgram, state::*, state::signer_v2::ExtraVerificationData, state::signer_v2::precompile::create_sync_consensus_message, utils::*, SmallVec};

/// Arguments for synchronous settings transaction
///
/// # BREAKING CHANGE (v2)
/// `num_signers` now represents the TOTAL count of ALL signers (native + external).
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SyncSettingsTransactionArgs {
    /// Total count of ALL signers (native + external) in remaining_accounts.
    /// Instructions sysvar must be at position num_signers if external signers present.
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
        &mut self,
        args: &SyncSettingsTransactionArgs,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<SmallVec<u8, ExtraVerificationData>>,
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

        // Build message for external signer verification.
        // Hash the actions payload so external signers commit to the exact settings changes.
        let actions_bytes = args.actions.try_to_vec()
            .map_err(|_| SmartAccountError::InvalidPayload)?;
        let payload_hash = hash(&actions_bytes);
        let message = create_sync_consensus_message(
            &consensus_account.key(),
            consensus_account.transaction_index(),
            &payload_hash.to_bytes(),
        );

        // Validates synchronous consensus across the signers
        let evd: &[ExtraVerificationData] = match &extra_verification_data {
            Some(v) => v,
            None => &[],
        };
        validate_synchronous_consensus(consensus_account, args.num_signers, remaining_accounts, message, evd)?;

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, None))]
    pub fn sync_settings_transaction(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncSettingsTransactionArgs,
    ) -> Result<()> {
        Self::sync_settings_transaction_inner(ctx, args)
    }

    /// Sync settings transaction with V2 signer support.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, extra_verification_data))]
    pub fn sync_settings_transaction_v2(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncSettingsTransactionArgs,
        extra_verification_data: Option<SmallVec<u8, ExtraVerificationData>>,
    ) -> Result<()> {
        Self::sync_settings_transaction_inner(ctx, args)
    }

    fn sync_settings_transaction_inner(
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
            &settings.signers,
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
            settings: settings.clone(),
            changes: args.actions.clone(),
        };

        SmartAccountEvent::SynchronousSettingsTransactionEvent(event).log(&log_authority_info)?;

        Ok(())
    }
}
