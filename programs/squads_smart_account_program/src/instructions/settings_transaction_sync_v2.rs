use anchor_lang::prelude::*;

use crate::{
    consensus::ConsensusAccount,
    consensus_trait::{Consensus, ConsensusAccountType},
    errors::*,
    events::*,
    program::SquadsSmartAccountProgram,
    state::*,
    utils::{collect_v2_signer_pubkeys, validate_settings_actions, validate_synchronous_consensus_v2, SyncConsensusV2Args, SyncConsensusV2Result},
};

/// Args for V2 synchronous settings transaction with external signer support
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SyncSettingsTransactionV2Args {
    /// Number of native signers (directly signing the transaction)
    pub num_native_signers: u8,
    /// Key IDs of external signers (verified via precompile)
    pub external_signer_key_ids: Vec<Pubkey>,
    /// Client data params for WebAuthn verification (required if any WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
    /// The settings actions to execute
    pub actions: Vec<SettingsAction>,
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct SyncSettingsTransactionV2<'info> {
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
    // 1. Instructions sysvar (if external signers are used)
    // 2. Native signer accounts (num_native_signers count)
    // 3. Any SpendingLimit accounts that need to be initialized/closed based on actions
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl<'info> SyncSettingsTransactionV2<'info> {
    fn validate(
        &self,
        args: &SyncSettingsTransactionV2Args,
        remaining_accounts: &[AccountInfo],
    ) -> Result<SyncConsensusV2Result> {
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

        // Build consensus args
        let consensus_args = SyncConsensusV2Args {
            num_native_signers: args.num_native_signers,
            external_signer_key_ids: args.external_signer_key_ids.clone(),
            client_data_params: args.client_data_params,
        };

        // Validates V2 synchronous consensus with external signer support
        let consensus_result = validate_synchronous_consensus_v2(
            &consensus_account,
            &consensus_args,
            consensus_account.key(),
            remaining_accounts,
        )?;

        Ok(consensus_result)
    }

    pub fn sync_settings_transaction_v2(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncSettingsTransactionV2Args,
    ) -> Result<()> {
        // Validate and get consensus result
        let consensus_result = ctx.accounts.validate(&args, &ctx.remaining_accounts)?;

        // Wrapper consensus account
        let consensus_account = &mut ctx.accounts.consensus_account;

        // Apply WebAuthn counter updates to prevent replay attacks
        consensus_account.apply_counter_updates(&consensus_result.counter_updates)?;
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

        // Get remaining accounts after consensus accounts
        let remaining_after_consensus = &ctx.remaining_accounts[consensus_result.accounts_consumed..];

        // Execute the actions one by one
        for action in args.actions.iter() {
            settings.modify_with_action(
                &settings_key,
                action,
                &rent,
                &ctx.accounts.rent_payer,
                &ctx.accounts.system_program,
                &remaining_after_consensus,
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

        // Collect signer pubkeys for event (native + external)
        let signer_pubkeys = collect_v2_signer_pubkeys(
            args.num_native_signers,
            &args.external_signer_key_ids,
            &ctx.remaining_accounts,
        );

        // Log the event
        let event = SynchronousSettingsTransactionEvent {
            settings_pubkey: settings_key,
            signers: signer_pubkeys,
            settings: settings.clone(),
            changes: args.actions.clone(),
        };

        SmartAccountEvent::SynchronousSettingsTransactionEvent(event).log(&log_authority_info)?;

        Ok(())
    }
}
