//! `SettingsChangePolicy` — pure types + `From<LimitedSettingsAction>` live
//! in the types crate. Program-local: `PolicyTrait` impl and account
//! validation.

use anchor_lang::prelude::*;

pub use squads_smart_account_program_types::{
    AllowedSettingsChange, LimitedSettingsAction, SettingsChangeExecutionArgs, SettingsChangePayload,
    SettingsChangePolicy, SettingsChangePolicyCreationPayload,
};

use crate::error_conv::ToAnchorResult;
use crate::program::SquadsSmartAccountProgram;
use crate::{
    errors::*,
    events::*,
    get_settings_signer_seeds,
    state::{policies::policy_core::{PolicyExecutionContext, PolicyTrait}, SettingsExt},
    Settings, SettingsAction,
};

pub struct ValidatedAccounts<'info> {
    pub settings: Account<'info, Settings>,
    /// Optional just to comply with later use of Settings::modify_with_action
    pub rent_payer: Option<Signer<'info>>,
    /// Optional just to comply with later use of Settings::modify_with_action
    pub system_program: Option<Program<'info, System>>,
    /// Program account used for logging
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

/// Anchor-returning validate_payload dispatch helper.
pub trait SettingsChangePolicyExt {
    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &SettingsChangePayload,
    ) -> Result<()>;
}

impl SettingsChangePolicyExt for SettingsChangePolicy {
    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &SettingsChangePayload,
    ) -> Result<()> {
        SettingsChangePolicy::validate_payload(self, context, payload).to_anchor()
    }
}

// =============================================================================
// POLICY TRAIT IMPLEMENTATION
// =============================================================================
impl PolicyTrait for SettingsChangePolicy {
    type PolicyState = Self;
    type CreationPayload = SettingsChangePolicyCreationPayload;
    type UsagePayload = SettingsChangePayload;
    type ExecutionArgs = SettingsChangeExecutionArgs;

    /// Validate policy invariants - no duplicate actions
    fn invariant(&self) -> Result<()> {
        SettingsChangePolicy::invariant(self).to_anchor()
    }

    /// Validate that the payload actions match allowed policy actions
    fn validate_payload(
        &self,
        // No difference between synchronous and asynchronous execution
        context: PolicyExecutionContext,
        payload: &Self::UsagePayload,
    ) -> Result<()> {
        SettingsChangePolicy::validate_payload(self, context, payload).to_anchor()
    }

    /// Execute the settings change actions
    fn execute_payload<'info>(
        &mut self,
        args: Self::ExecutionArgs,
        payload: &Self::UsagePayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        // Validate and grab the settings account
        let mut validated_accounts = validate_accounts(args.settings_key, accounts)?;
        for action in payload.actions.iter() {
            let settings_action = SettingsAction::from(action.clone());
            validated_accounts.settings.modify_with_action(
                &args.settings_key,
                &settings_action,
                &Rent::get()?,
                &validated_accounts.rent_payer,
                &validated_accounts.system_program,
                // Only policies and spending limits use remaining accounts, and
                // those actions are excluded from LimitedSettingsAction
                &[],
                &crate::ID,
                None,
            )?;

            // Run settings invariant
            validated_accounts.settings.invariant().to_anchor()?;

            let log_authority_info = LogAuthorityInfo {
                authority: validated_accounts.settings.to_account_info(),
                authority_seeds: get_settings_signer_seeds(validated_accounts.settings.seed),
                bump: validated_accounts.settings.bump,
                program: validated_accounts.program.to_account_info(),
            };

            // Log the event since we're modifying the settings state
            let event = SettingsChangePolicyEvent {
                settings_pubkey: validated_accounts.settings.key(),
                settings: validated_accounts.settings.clone().into_inner(),
                changes: payload.actions.clone(),
            };
            SmartAccountEvent::SettingsChangePolicyEvent(event).log(&log_authority_info)?;
        }
        // Reallocate the settings account if needed
        Settings::realloc_if_needed(
            validated_accounts.settings.to_account_info(),
            validated_accounts.settings.signers.len(),
            validated_accounts
                .rent_payer
                .map(|rent_payer| rent_payer.to_account_info()),
            validated_accounts
                .system_program
                .map(|system_program| system_program.to_account_info()),
        )?;
        Ok(())
    }
}

pub fn validate_accounts<'info>(
    settings_key: Pubkey,
    accounts: &'info [AccountInfo<'info>],
) -> Result<ValidatedAccounts<'info>> {
    let (settings_account_info, rent_payer_info, system_program_info, program_info) =
        if let [settings_account_info, rent_payer_info, system_program_info, program_info, _remaining @ ..] =
            accounts
        {
            (settings_account_info, rent_payer_info, system_program_info, program_info)
        } else {
            return err!(SmartAccountError::InvalidNumberOfAccounts);
        };

    require!(
        settings_account_info.key() == settings_key,
        SmartAccountError::SettingsChangeInvalidSettingsKey
    );
    require!(
        settings_account_info.is_writable,
        SmartAccountError::SettingsChangeInvalidSettingsAccount
    );
    let settings: Account<'info, Settings> = Account::try_from(settings_account_info)?;

    require!(
        settings.settings_authority == Pubkey::default(),
        SmartAccountError::NotSupportedForControlled
    );

    let rent_payer = Signer::try_from(rent_payer_info)
        .map_err(|_| SmartAccountError::SettingsChangeInvalidRentPayer)?;
    require!(
        rent_payer.is_writable,
        SmartAccountError::SettingsChangeInvalidRentPayer
    );

    let system_program: Program<'info, System> = Program::try_from(system_program_info)
        .map_err(|_| SmartAccountError::SettingsChangeInvalidSystemProgram)?;

    let program: Program<'info, SquadsSmartAccountProgram> =
        Program::try_from(program_info).map_err(|_| SmartAccountError::InvalidAccount)?;

    Ok(ValidatedAccounts {
        settings,
        rent_payer: Some(rent_payer),
        system_program: Some(system_program),
        program,
    })
}
