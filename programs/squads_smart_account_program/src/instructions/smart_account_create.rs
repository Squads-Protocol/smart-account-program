#![allow(deprecated)]
use std::borrow::Borrow;

use account_events::CreateSmartAccountEvent;
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use solana_program::native_token::LAMPORTS_PER_SOL;

use crate::errors::SmartAccountError;
use crate::events::*;
use crate::program::SquadsSmartAccountProgram;
use crate::state::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateSmartAccountArgs {
    /// The authority that can configure the smart account: add/remove signers, change the threshold, etc.
    /// Should be set to `None` for autonomous smart accounts.
    pub settings_authority: Option<Pubkey>,
    /// The number of signatures required to execute a transaction.
    pub threshold: u16,
    /// The signers on the smart account.
    pub signers: Vec<SmartAccountSigner>,
    /// How many seconds must pass between transaction voting, settlement, and execution.
    pub time_lock: u32,
    /// The address where the rent for the accounts related to executed, rejected, or cancelled
    /// transactions can be reclaimed. If set to `None`, the rent reclamation feature is turned off.
    pub rent_collector: Option<Pubkey>,
    /// Memo is used for indexing only.
    pub memo: Option<String>,
}

#[derive(Accounts)]
#[instruction(args: CreateSmartAccountArgs)]
pub struct CreateSmartAccount<'info> {
    /// Global program config account.
    #[account(mut, seeds = [SEED_PREFIX, SEED_PROGRAM_CONFIG], bump)]
    pub program_config: Account<'info, ProgramConfig>,

    /// The treasury where the creation fee is transferred to.
    /// CHECK: validation is performed in the `MultisigCreate::validate()` method.
    #[account(mut)]
    pub treasury: AccountInfo<'info>,

    /// The creator of the smart account.
    #[account(mut)]
    pub creator: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl<'info> CreateSmartAccount<'info> {
    fn validate(&self) -> Result<()> {
        //region treasury
        require_keys_eq!(
            self.treasury.key(),
            self.program_config.treasury,
            SmartAccountError::InvalidAccount
        );
        //endregion

        Ok(())
    }

    /// Creates a multisig.
    #[access_control(ctx.accounts.validate())]
    pub fn create_smart_account(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: CreateSmartAccountArgs,
    ) -> Result<()> {
        let program_config = &mut ctx.accounts.program_config;
        // Sort the members by pubkey.
        let mut signers = args.signers;
        signers.sort_by_key(|m| m.key);

        let settings_seed = program_config.smart_account_index.checked_add(1).unwrap();
        let (settings_pubkey, settings_bump) = Pubkey::find_program_address(
            &[
                SEED_PREFIX,
                SEED_SETTINGS,
                settings_seed.to_le_bytes().as_ref(),
            ],
            &crate::ID,
        );
        // Initialize the settings
        let settings_configuration = Settings {
            seed: settings_seed,
            settings_authority: args.settings_authority.unwrap_or_default(),
            threshold: args.threshold,
            time_lock: args.time_lock,
            transaction_index: 0,
            stale_transaction_index: 0,
            // Preset to Pubkey::default() until archival feature is implemented.
            archival_authority: Some(Pubkey::default()),
            // Preset to 0 until archival feature is implemented.
            archivable_after: 0,
            bump: settings_bump,
            signers,
            account_utilization: 0,
            _reserved1: 0,
            _reserved2: 0,
        };

        // Initialize the settings account with the configuration.
        let settings_account_info = settings_configuration.find_and_initialize_settings_account(
            settings_pubkey,
            &ctx.accounts.creator.to_account_info(),
            &ctx.remaining_accounts,
            &ctx.accounts.system_program,
        )?;
        // Serialize the settings account.
        settings_configuration
            .try_serialize(&mut &mut settings_account_info.data.borrow_mut()[..])?;

        settings_configuration.invariant()?;

        // Check if the creation fee is set and transfer the fee to the treasury if necessary.
        let creation_fee = program_config.smart_account_creation_fee;

        if creation_fee > 0 {
            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: ctx.accounts.creator.to_account_info(),
                        to: ctx.accounts.treasury.to_account_info(),
                    },
                ),
                creation_fee,
            )?;
            msg!("Creation fee: {}", creation_fee / LAMPORTS_PER_SOL);
        }

        // Increment the smart account index.
        program_config.increment_smart_account_index()?;

        // Log Smart Account Creation
        let event = CreateSmartAccountEvent {
            new_settings_pubkey: settings_pubkey,
            new_settings_content: settings_configuration.clone(),
        };
        let log_authority_info = LogAuthorityInfo {
            authority: settings_account_info.clone(),
            authority_seeds: get_settings_signer_seeds(settings_seed),
            bump: settings_bump,
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::CreateSmartAccountEvent(event).log(&log_authority_info)?;

        Ok(())
    }
}
