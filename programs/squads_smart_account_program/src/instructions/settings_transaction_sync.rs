use anchor_lang::prelude::*;

use crate::{errors::*, id, state::*, utils::*};

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
        seeds = [SEED_PREFIX, SEED_SETTINGS, settings.seed.as_ref()],
        bump = settings.bump,
    )]
    pub settings: Box<Account<'info, Settings>>,

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
}

impl<'info> SyncSettingsTransaction<'info> {
    fn validate(
        &self,
        args: &SyncSettingsTransactionArgs,
        remaining_accounts: &[AccountInfo],
    ) -> Result<()> {
        let Self { settings, .. } = self;

        // Settings must not be controlled
        require_keys_eq!(
            settings.settings_authority,
            Pubkey::default(),
            SmartAccountError::NotSupportedForControlled
        );

        // Settings must not be time locked
        require_eq!(settings.time_lock, 0, SmartAccountError::TimeLockNotZero);

        // Settings transaction must have at least one action
        require!(!args.actions.is_empty(), SmartAccountError::NoActions);

        // new time_lock must not exceed the maximum allowed
        for action in &args.actions {
            if let SettingsAction::SetTimeLock { new_time_lock } = action {
                require!(
                    *new_time_lock <= MAX_TIME_LOCK,
                    SmartAccountError::TimeLockExceedsMaxAllowed
                );
            }
        }

        // Get signers from remaining accounts using threshold
        let required_signer_count = settings.threshold as usize;
        let signer_count = args.num_signers as usize;
        require!(
            signer_count >= required_signer_count,
            SmartAccountError::InvalidSignerCount
        );

        let signers = remaining_accounts
            .get(..signer_count)
            .ok_or(SmartAccountError::InvalidSignerCount)?;

        // Setup the aggregated permissions and the vote permission count
        let mut aggregated_permissions = Permissions { mask: 0 };
        let mut vote_permission_count = 0;
        let mut seen_members = Vec::with_capacity(signer_count);

        // Check permissions for all signers
        for signer in signers.iter() {
            if let Some(member_index) = settings.is_signer(signer.key()) {
                // Check that the signer is indeed a signer
                if !signer.is_signer {
                    return err!(SmartAccountError::MissingSignature);
                }
                // Check for duplicate signer
                if seen_members.contains(&signer.key()) {
                    return err!(SmartAccountError::DuplicateSigner);
                }
                seen_members.push(signer.key());

                let member_permissions = settings.signers[member_index].permissions;
                // Add to the aggregated permissions mask
                aggregated_permissions.mask |= member_permissions.mask;

                // Count the vote permissions
                if member_permissions.has(Permission::Vote) {
                    vote_permission_count += 1;
                }
            } else {
                return err!(SmartAccountError::NotASigner);
            }
        }

        // Check if we have all required permissions (Initiate | Vote | Execute = 7)
        require!(
            aggregated_permissions.mask == 7,
            SmartAccountError::InsufficientAggregatePermissions
        );

        // Verify threshold is met across all voting permissions
        require!(
            vote_permission_count >= settings.threshold as usize,
            SmartAccountError::InsufficientVotePermissions
        );

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts))]
    pub fn sync_settings_transaction(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncSettingsTransactionArgs,
    ) -> Result<()> {
        let settings = &mut ctx.accounts.settings;
        let rent = Rent::get()?;

        // Execute the actions one by one
        for action in args.actions.iter() {
            match action {
                SettingsAction::AddSigner { new_signer } => {
                    settings.add_signer(new_signer.to_owned());

                    settings.invalidate_prior_transactions();
                }

                SettingsAction::RemoveSigner { old_signer } => {
                    settings.remove_signer(old_signer.to_owned())?;

                    settings.invalidate_prior_transactions();
                }

                SettingsAction::ChangeThreshold { new_threshold } => {
                    settings.threshold = *new_threshold;

                    settings.invalidate_prior_transactions();
                }

                SettingsAction::SetTimeLock { new_time_lock } => {
                    settings.time_lock = *new_time_lock;

                    settings.invalidate_prior_transactions();
                }

                SettingsAction::AddSpendingLimit {
                    seed,
                    account_index,
                    signers,
                    mint,
                    amount,
                    period,
                    destinations,
                    expiration,
                } => {
                    let (spending_limit_key, spending_limit_bump) = Pubkey::find_program_address(
                        &[
                            SEED_PREFIX,
                            settings.key().as_ref(),
                            SEED_SPENDING_LIMIT,
                            seed.as_ref(),
                        ],
                        ctx.program_id,
                    );

                    // Find the SpendingLimit account in `remaining_accounts`.
                    let spending_limit_info = ctx
                        .remaining_accounts
                        .iter()
                        .find(|acc| acc.key == &spending_limit_key)
                        .ok_or(SmartAccountError::MissingAccount)?;

                    // `rent_payer` and `system_program` must also be present.
                    let rent_payer = &ctx
                        .accounts
                        .rent_payer
                        .as_ref()
                        .ok_or(SmartAccountError::MissingAccount)?;
                    let system_program = &ctx
                        .accounts
                        .system_program
                        .as_ref()
                        .ok_or(SmartAccountError::MissingAccount)?;

                    // Initialize the SpendingLimit account.
                    create_account(
                        rent_payer,
                        spending_limit_info,
                        system_program,
                        &id(),
                        &rent,
                        SpendingLimit::size(signers.len(), destinations.len()),
                        vec![
                            SEED_PREFIX.to_vec(),
                            settings.key().as_ref().to_vec(),
                            SEED_SPENDING_LIMIT.to_vec(),
                            seed.as_ref().to_vec(),
                            vec![spending_limit_bump],
                        ],
                    )?;

                    let mut signers = signers.to_vec();
                    // Make sure signers are sorted.
                    signers.sort();

                    // Serialize the SpendingLimit data into the account info.
                    let spending_limit = SpendingLimit {
                        settings: settings.key().to_owned(),
                        seed: seed.to_owned(),
                        account_index: *account_index,
                        signers,
                        amount: *amount,
                        mint: *mint,
                        period: *period,
                        remaining_amount: *amount,
                        last_reset: Clock::get()?.unix_timestamp,
                        bump: spending_limit_bump,
                        destinations: destinations.to_vec(),
                        expiration: *expiration,
                    };

                    spending_limit.invariant()?;

                    spending_limit
                        .try_serialize(&mut &mut spending_limit_info.data.borrow_mut()[..])?;
                }

                SettingsAction::RemoveSpendingLimit {
                    spending_limit: spending_limit_key,
                } => {
                    // Find the SpendingLimit account in `remaining_accounts`.
                    let spending_limit_info = ctx
                        .remaining_accounts
                        .iter()
                        .find(|acc| acc.key == spending_limit_key)
                        .ok_or(SmartAccountError::MissingAccount)?;

                    // `rent_payer` must also be present.
                    let rent_payer = &ctx
                        .accounts
                        .rent_payer
                        .as_ref()
                        .ok_or(SmartAccountError::MissingAccount)?;

                    let spending_limit = Account::<SpendingLimit>::try_from(spending_limit_info)?;

                    // SpendingLimit must belong to the `settings`.
                    require_keys_eq!(
                        spending_limit.settings,
                        settings.key(),
                        SmartAccountError::InvalidAccount
                    );

                    spending_limit.close(rent_payer.to_account_info())?;

                    // We don't need to invalidate prior transactions here because adding
                    // a spending limit doesn't affect the consensus parameters of the smart account.
                }

                SettingsAction::SetRentCollector { new_rent_collector } => {
                    settings.rent_collector = *new_rent_collector;

                    // We don't need to invalidate prior transactions here because changing
                    // `rent_collector` doesn't affect the consensus parameters of the smart account.
                }
            }
        }

        // Make sure the smart account can fit the updated state: added signers or newly set rent_collector.
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

        // Make sure the settings state is valid after applying the actions
        settings.invariant()?;

        Ok(())
    }
}
