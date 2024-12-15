use anchor_lang::prelude::*;

use crate::errors::*;
use crate::state::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddSignerArgs {
    pub new_signer: SmartAccountSigner,
    /// Memo is used for indexing only.
    pub memo: Option<String>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RemoveSignerArgs {
    pub old_signer: Pubkey,
    /// Memo is used for indexing only.
    pub memo: Option<String>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ChangeThresholdArgs {
    pub new_threshold: u16,
    /// Memo is used for indexing only.
    pub memo: Option<String>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SetTimeLockArgs {
    pub time_lock: u32,
    /// Memo is used for indexing only.
    pub memo: Option<String>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SetNewSettingsAuthorityArgs {
    pub new_settings_authority: Pubkey,
    /// Memo is used for indexing only.
    pub memo: Option<String>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SetRentCollectorArgs {
    pub rent_collector: Option<Pubkey>,
    /// Memo is used for indexing only.
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct ExecuteSettingsTransactionAsAuthority<'info> {
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.as_ref()],
        bump = settings.bump,
    )]
    settings: Account<'info, Settings>,

    /// Settings `settings_authority` that must authorize the configuration change.
    pub settings_authority: Signer<'info>,

    /// The account that will be charged or credited in case the multisig account needs to reallocate space,
    /// for example when adding a new member or a spending limit.
    /// This is usually the same as `config_authority`, but can be a different account if needed.
    #[account(mut)]
    pub rent_payer: Option<Signer<'info>>,

    /// We might need it in case reallocation is needed.
    pub system_program: Option<Program<'info, System>>,
}

impl ExecuteSettingsTransactionAsAuthority<'_> {
    fn validate(&self) -> Result<()> {
        require_keys_eq!(
            self.settings_authority.key(),
            self.settings.settings_authority,
            SmartAccountError::Unauthorized
        );

        Ok(())
    }

    /// Add a member/key to the multisig and reallocate space if necessary.
    ///
    /// NOTE: This instruction must be called only by the `config_authority` if one is set (Controlled Multisig).
    ///       Uncontrolled Mustisigs should use `config_transaction_create` instead.
    #[access_control(ctx.accounts.validate())]
    pub fn add_signer(ctx: Context<Self>, args: AddSignerArgs) -> Result<()> {
        let AddSignerArgs { new_signer, .. } = args;

        let settings = &mut ctx.accounts.settings;

        // Make sure that the new member is not already in the multisig.
        require!(
            settings.is_signer(new_signer.key).is_none(),
            SmartAccountError::DuplicateSigner
        );

        settings.add_signer(new_signer);

        // Make sure the multisig account can fit the newly set rent_collector.
        Settings::realloc_if_needed(
            settings.to_account_info(),
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

        settings.invalidate_prior_transactions();

        settings.invariant()?;

        Ok(())
    }

    /// Remove a member/key from the multisig.
    ///
    /// NOTE: This instruction must be called only by the `config_authority` if one is set (Controlled Multisig).
    ///       Uncontrolled Mustisigs should use `config_transaction_create` instead.
    #[access_control(ctx.accounts.validate())]
    pub fn remove_signer(ctx: Context<Self>, args: RemoveSignerArgs) -> Result<()> {
        let settings = &mut ctx.accounts.settings;

        require!(
            settings.signers.len() > 1,
            SmartAccountError::RemoveLastSigner
        );

        settings.remove_signer(args.old_signer)?;

        settings.invalidate_prior_transactions();

        settings.invariant()?;

        Ok(())
    }

    /// NOTE: This instruction must be called only by the `config_authority` if one is set (Controlled Multisig).
    ///       Uncontrolled Mustisigs should use `config_transaction_create` instead.
    #[access_control(ctx.accounts.validate())]
    pub fn change_threshold(ctx: Context<Self>, args: ChangeThresholdArgs) -> Result<()> {
        let ChangeThresholdArgs { new_threshold, .. } = args;

        let settings = &mut ctx.accounts.settings;

        settings.threshold = new_threshold;

        settings.invalidate_prior_transactions();

        settings.invariant()?;

        Ok(())
    }

    /// Set the `time_lock` config parameter for the multisig.
    ///
    /// NOTE: This instruction must be called only by the `config_authority` if one is set (Controlled Multisig).
    ///       Uncontrolled Mustisigs should use `config_transaction_create` instead.
    #[access_control(ctx.accounts.validate())]
    pub fn set_time_lock(ctx: Context<Self>, args: SetTimeLockArgs) -> Result<()> {
        let settings = &mut ctx.accounts.settings;

        settings.time_lock = args.time_lock;

        settings.invalidate_prior_transactions();

        settings.invariant()?;

        Ok(())
    }

    /// Set the multisig `config_authority`.
    ///
    /// NOTE: This instruction must be called only by the `config_authority` if one is set (Controlled Multisig).
    ///       Uncontrolled Mustisigs should use `config_transaction_create` instead.
    #[access_control(ctx.accounts.validate())]
    pub fn set_new_settings_authority(
        ctx: Context<Self>,
        args: SetNewSettingsAuthorityArgs,
    ) -> Result<()> {
        let settings = &mut ctx.accounts.settings;

        settings.settings_authority = args.new_settings_authority;

        settings.invalidate_prior_transactions();

        settings.invariant()?;

        Ok(())
    }

    /// Set the multisig `rent_collector` and reallocate space if necessary.
    ///
    /// NOTE: This instruction must be called only by the `config_authority` if one is set (Controlled Multisig).
    ///       Uncontrolled Mustisigs should use `config_transaction_create` instead.
    #[access_control(ctx.accounts.validate())]
    pub fn set_rent_collector(
        ctx: Context<Self>,
        args: SetRentCollectorArgs,
    ) -> Result<()> {
        let settings = &mut ctx.accounts.settings;

        settings.rent_collector = args.rent_collector;

        // Make sure the multisig account can fit the newly set rent_collector.
        Settings::realloc_if_needed(
            settings.to_account_info(),
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

        // We don't need to invalidate prior transactions here because changing
        // `rent_collector` doesn't affect the consensus parameters of the multisig.

        settings.invariant()?;

        Ok(())
    }
}
