use anchor_lang::prelude::*;

use crate::{errors::*, state::*, utils::*};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ConfigTransactionSyncArgs {
    /// The number of signers to reach threshold and adequate permissions
    pub num_signers: u8,
    /// The configuration actions to execute
    pub actions: Vec<ConfigAction>,
    pub memo: Option<String>,
}

#[derive(Accounts)]
pub struct ConfigTransactionSync<'info> {
    #[account(
        mut,
        seeds = [SEED_PREFIX, SEED_MULTISIG, multisig.create_key.as_ref()],
        bump = multisig.bump,
    )]
    pub multisig: Box<Account<'info, Multisig>>,

    /// The account that will be charged/credited in case the config transaction causes space reallocation,
    /// for example when adding a new member, adding or removing a spending limit.
    /// This is usually the same as `member`, but can be a different account if needed.
    #[account(mut)]
    pub rent_payer: Option<Signer<'info>>,

    /// We might need it in case reallocation is needed.
    pub system_program: Option<Program<'info, System>>,
    // `remaining_accounts` must include the following accounts in the exact order:
    // 1. The exact amount of signers required to reach the threshold
    // 2. Any SpendingLimit accounts that need to be initialized/closed based on actions
}

impl<'info> ConfigTransactionSync<'info> {
    fn validate(
        &self,
        args: &ConfigTransactionSyncArgs,
        remaining_accounts: &[AccountInfo],
    ) -> Result<()> {
        let Self { multisig, .. } = self;

        // Multisig must not be controlled
        require_keys_eq!(
            multisig.config_authority,
            Pubkey::default(),
            MultisigError::NotSupportedForControlled
        );

        // Multisig must not be time locked
        require_eq!(multisig.time_lock, 0, MultisigError::TimeLockNotZero);

        // Config transaction must have at least one action
        require!(!args.actions.is_empty(), MultisigError::NoActions);

        // time_lock must not exceed the maximum allowed
        for action in &args.actions {
            if let ConfigAction::SetTimeLock { new_time_lock } = action {
                require!(
                    *new_time_lock <= MAX_TIME_LOCK,
                    MultisigError::TimeLockExceedsMaxAllowed
                );
            }
        }

        // Get signers from remaining accounts using threshold
        let required_signer_count = multisig.threshold as usize;
        let signer_count = args.num_signers as usize;
        require!(
            signer_count >= required_signer_count,
            MultisigError::InvalidSignerCount
        );

        let signers = remaining_accounts
            .get(..signer_count)
            .ok_or(MultisigError::InvalidSignerCount)?;

        msg!("Signers contributing to Consensus:");
        for (index, signer) in signers.iter().enumerate() {
            msg!("#{:?}: {:?}", index, signer.key());
        }

        // Setup the aggregated permissions and the vote permission count
        let mut aggregated_permissions = Permissions { mask: 0 };
        let mut vote_permission_count = 0;
        let mut seen_members = Vec::with_capacity(signer_count);

        // Check permissions for all signers
        for signer in signers.iter() {
            if let Some(member_index) = multisig.is_member(signer.key()) {
                // Check that the signer is indeed a signer
                if !signer.is_signer {
                    return err!(MultisigError::MissingSignature);
                }
                // Check for duplicate signer
                if seen_members.contains(&signer.key()) {
                    return err!(MultisigError::DuplicateMember);
                }
                seen_members.push(signer.key());

                let member_permissions = multisig.members[member_index].permissions;
                // Add to the aggregated permissions mask
                aggregated_permissions.mask |= member_permissions.mask;

                // Count the vote permissions
                if member_permissions.has(Permission::Vote) {
                    vote_permission_count += 1;
                }
            } else {
                return err!(MultisigError::NotAMember);
            }
        }

        // Check if we have all required permissions (Initiate | Vote | Execute = 7)
        msg!(
            "Aggregate Permissions Mask: {:?}",
            aggregated_permissions.mask
        );
        require!(
            aggregated_permissions.mask == 7,
            MultisigError::InsufficientAggregatePermissions
        );

        // Verify threshold is met across all voting permissions
        require!(
            vote_permission_count >= multisig.threshold as usize,
            MultisigError::InsufficientVotePermissions
        );

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts))]
    pub fn config_transaction_sync(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: ConfigTransactionSyncArgs,
    ) -> Result<()> {
        let multisig = &mut ctx.accounts.multisig;
        let rent = Rent::get()?;

        // Execute the actions one by one
        for action in args.actions.iter() {
            match action {
                ConfigAction::AddMember { new_member } => {
                    multisig.add_member(new_member.to_owned());

                    multisig.invalidate_prior_transactions();
                }

                ConfigAction::RemoveMember { old_member } => {
                    multisig.remove_member(old_member.to_owned())?;

                    multisig.invalidate_prior_transactions();
                }

                ConfigAction::ChangeThreshold { new_threshold } => {
                    multisig.threshold = *new_threshold;

                    multisig.invalidate_prior_transactions();
                }

                ConfigAction::SetTimeLock { new_time_lock } => {
                    multisig.time_lock = *new_time_lock;

                    multisig.invalidate_prior_transactions();
                }

                ConfigAction::AddSpendingLimit {
                    create_key,
                    vault_index,
                    mint,
                    amount,
                    period,
                    members,
                    destinations,
                } => {
                    let (spending_limit_key, spending_limit_bump) = Pubkey::find_program_address(
                        &[
                            SEED_PREFIX,
                            multisig.key().as_ref(),
                            SEED_SPENDING_LIMIT,
                            create_key.as_ref(),
                        ],
                        ctx.program_id,
                    );

                    // Find the SpendingLimit account in `remaining_accounts`.
                    let spending_limit_info = ctx
                        .remaining_accounts
                        .iter()
                        .find(|acc| acc.key == &spending_limit_key)
                        .ok_or(MultisigError::MissingAccount)?;

                    // `rent_payer` and `system_program` must also be present.
                    let rent_payer = &ctx
                        .accounts
                        .rent_payer
                        .as_ref()
                        .ok_or(MultisigError::MissingAccount)?;
                    let system_program = &ctx
                        .accounts
                        .system_program
                        .as_ref()
                        .ok_or(MultisigError::MissingAccount)?;

                    // Initialize the SpendingLimit account.
                    create_account(
                        rent_payer,
                        spending_limit_info,
                        system_program,
                        &crate::id(),
                        &rent,
                        SpendingLimit::size(members.len(), destinations.len()),
                        vec![
                            SEED_PREFIX.to_vec(),
                            multisig.key().as_ref().to_vec(),
                            SEED_SPENDING_LIMIT.to_vec(),
                            create_key.as_ref().to_vec(),
                            vec![spending_limit_bump],
                        ],
                    )?;

                    let mut members = members.to_vec();
                    // Make sure members are sorted.
                    members.sort();

                    // Serialize the SpendingLimit data into the account info.
                    let spending_limit = SpendingLimit {
                        multisig: multisig.key().to_owned(),
                        create_key: create_key.to_owned(),
                        vault_index: *vault_index,
                        amount: *amount,
                        mint: *mint,
                        period: *period,
                        remaining_amount: *amount,
                        last_reset: Clock::get()?.unix_timestamp,
                        bump: spending_limit_bump,
                        members,
                        destinations: destinations.to_vec(),
                    };

                    spending_limit.invariant()?;

                    spending_limit
                        .try_serialize(&mut &mut spending_limit_info.data.borrow_mut()[..])?;
                }

                ConfigAction::RemoveSpendingLimit {
                    spending_limit: spending_limit_key,
                } => {
                    // Find the SpendingLimit account in `remaining_accounts`.
                    let spending_limit_info = ctx
                        .remaining_accounts
                        .iter()
                        .find(|acc| acc.key == spending_limit_key)
                        .ok_or(MultisigError::MissingAccount)?;

                    // `rent_payer` must also be present.
                    let rent_payer = &ctx
                        .accounts
                        .rent_payer
                        .as_ref()
                        .ok_or(MultisigError::MissingAccount)?;

                    let spending_limit = Account::<SpendingLimit>::try_from(spending_limit_info)?;

                    // SpendingLimit must belong to the `multisig`.
                    require_keys_eq!(
                        spending_limit.multisig,
                        multisig.key(),
                        MultisigError::InvalidAccount
                    );

                    spending_limit.close(rent_payer.to_account_info())?;

                    // We don't need to invalidate prior transactions here because adding
                    // a spending limit doesn't affect the consensus parameters of the multisig.
                }

                ConfigAction::SetRentCollector { new_rent_collector } => {
                    multisig.rent_collector = *new_rent_collector;

                    // We don't need to invalidate prior transactions here because changing
                    // `rent_collector` doesn't affect the consensus parameters of the multisig.
                }
            }
        }

        // Make sure the multisig account can fit the updated state: added members or newly set rent_collector.
        Multisig::realloc_if_needed(
            multisig.to_account_info(),
            multisig.members.len(),
            ctx.accounts
                .rent_payer
                .as_ref()
                .map(ToAccountInfo::to_account_info),
            ctx.accounts
                .system_program
                .as_ref()
                .map(ToAccountInfo::to_account_info),
        )?;

        // Make sure the multisig state is valid after applying the actions
        multisig.invariant()?;

        Ok(())
    }
}
