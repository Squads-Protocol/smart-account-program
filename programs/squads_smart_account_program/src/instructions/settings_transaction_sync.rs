use anchor_lang::prelude::*;

use crate::{
    consensus::ConsensusAccount,
    consensus_trait::{Consensus, ConsensusAccountType},
    errors::*,
    events::*,
    program::SquadsSmartAccountProgram,
    state::*,
    utils::{
        collect_v2_signer_pubkeys, validate_settings_actions, validate_synchronous_consensus,
        validate_synchronous_consensus_v2, SyncConsensusV2Args, SyncConsensusV2Result,
    },
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SyncSettingsTransactionArgs {
    /// The number of signers to reach threshold and adequate permissions
    pub num_signers: u8,
    /// The settings actions to execute
    pub actions: Vec<SettingsAction>,
    pub memo: Option<String>,
}

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
        remaining_accounts: &'info [AccountInfo<'info>],
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

    fn validate_v2(
        &self,
        args: &SyncSettingsTransactionV2Args,
        remaining_accounts: &'info [AccountInfo<'info>],
    ) -> Result<SyncConsensusV2Result> {
        let Self { consensus_account, .. } = self;
        let settings = consensus_account.read_only_settings()?;

        require_keys_eq!(
            settings.settings_authority,
            Pubkey::default(),
            SmartAccountError::NotSupportedForControlled
        );

        validate_settings_actions(&args.actions)?;

        let consensus_args = SyncConsensusV2Args {
            num_native_signers: args.num_native_signers,
            external_signer_key_ids: args.external_signer_key_ids.clone(),
            client_data_params: args.client_data_params,
        };

        let consensus_result = validate_synchronous_consensus_v2(
            &consensus_account,
            &consensus_args,
            consensus_account.key(),
            remaining_accounts,
        )?;

        Ok(consensus_result)
    }

    fn execute_inner(
        consensus_account: &mut Box<InterfaceAccount<'info, ConsensusAccount>>,
        actions: &[SettingsAction],
        rent_payer: &Option<Signer<'info>>,
        system_program: &Option<Program<'info, System>>,
        remaining_accounts: &'info [AccountInfo<'info>],
        signer_pubkeys: Vec<Pubkey>,
        program: &Program<'info, SquadsSmartAccountProgram>,
        program_id: &Pubkey,
    ) -> Result<()> {
        let settings_key = consensus_account.key();
        let settings_account_info = consensus_account.to_account_info();
        let settings = consensus_account.settings()?;
        let rent = Rent::get()?;

        let log_authority_info = LogAuthorityInfo {
            authority: settings_account_info.clone(),
            authority_seeds: get_settings_signer_seeds(settings.seed),
            bump: settings.bump,
            program: program.to_account_info(),
        };

        for action in actions.iter() {
            settings.modify_with_action(
                &settings_key,
                action,
                &rent,
                rent_payer,
                system_program,
                remaining_accounts,
                program_id,
                Some(&log_authority_info),
            )?;
        }

        Settings::realloc_if_needed(
            settings_account_info,
            settings.signers.len(),
            rent_payer.as_ref().map(ToAccountInfo::to_account_info),
            system_program.as_ref().map(ToAccountInfo::to_account_info),
        )?;

        settings.invariant()?;

        let event = SynchronousSettingsTransactionEvent {
            settings_pubkey: settings_key,
            signers: signer_pubkeys,
            settings: settings.clone(),
            changes: actions.to_vec(),
        };

        SmartAccountEvent::SynchronousSettingsTransactionEvent(event).log(&log_authority_info)?;

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts))]
    pub fn sync_settings_transaction(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncSettingsTransactionArgs,
    ) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let signer_pubkeys = ctx.remaining_accounts[..args.num_signers as usize]
            .iter()
            .map(|acc| *acc.key)
            .collect::<Vec<_>>();

        Self::execute_inner(
            consensus_account,
            &args.actions,
            &ctx.accounts.rent_payer,
            &ctx.accounts.system_program,
            &ctx.remaining_accounts,
            signer_pubkeys,
            &ctx.accounts.program,
            &ctx.program_id,
        )
    }

    #[access_control(ctx.accounts.validate_v2(&args, &ctx.remaining_accounts))]
    pub fn sync_settings_transaction_v2(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncSettingsTransactionV2Args,
    ) -> Result<()> {
        let consensus_result = ctx.accounts.validate_v2(&args, &ctx.remaining_accounts)?;
        let consensus_account = &mut ctx.accounts.consensus_account;

        consensus_account.apply_counter_updates(&consensus_result.counter_updates)?;

        let remaining_after_consensus = &ctx.remaining_accounts[consensus_result.accounts_consumed..];
        let signer_pubkeys = collect_v2_signer_pubkeys(
            args.num_native_signers,
            &args.external_signer_key_ids,
            &ctx.remaining_accounts,
        );

        Self::execute_inner(
            consensus_account,
            &args.actions,
            &ctx.accounts.rent_payer,
            &ctx.accounts.system_program,
            remaining_after_consensus,
            signer_pubkeys,
            &ctx.accounts.program,
            &ctx.program_id,
        )
    }
}
