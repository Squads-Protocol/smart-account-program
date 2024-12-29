#![allow(deprecated)]
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use solana_program::native_token::LAMPORTS_PER_SOL;

use crate::errors::SmartAccountError;
use crate::state::*;



/// These are only used to prevent the DOS vector of front running txns that
/// incrememnt the smart account index.
/// They will be removed once compression/archival is implemented.
#[cfg(feature = "testing")]
const ACCOUNT_CREATION_AUTHORITY: [u8; 32] = [
    152, 165, 37, 245, 229, 240, 130, 196, 233, 36, 234, 92, 142, 236, 214, 104, 221, 210, 13, 223,
    131, 100, 240, 8, 247, 125, 70, 118, 31, 150, 70, 126,
];

#[cfg(not(feature = "testing"))]
const ACCOUNT_CREATION_AUTHORITY: [u8; 32] = [
    92, 31, 87, 5, 157, 232, 219, 156, 230, 146, 81, 200, 219, 20, 50, 127, 26, 18, 84, 147, 206,
    244, 197, 115, 68, 27, 220, 156, 253, 92, 79, 64,
];


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
    #[account(seeds = [SEED_PREFIX, SEED_PROGRAM_CONFIG], bump)]
    pub program_config: Account<'info, ProgramConfig>,

    /// The treasury where the creation fee is transferred to.
    /// CHECK: validation is performed in the `MultisigCreate::validate()` method.
    #[account(mut)]
    pub treasury: AccountInfo<'info>,

    #[account(
        init,
        payer = creator,
        space = Settings::size(args.signers.len()),
        seeds = [SEED_PREFIX, SEED_SETTINGS, program_config.smart_account_index.checked_add(1).unwrap().to_le_bytes().as_ref()],
        bump
    )]
    pub settings: Account<'info, Settings>,

    /// The creator of the smart account.
    #[account(mut,
    address = Pubkey::from(ACCOUNT_CREATION_AUTHORITY))]
    pub creator: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl CreateSmartAccount<'_> {
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
    pub fn create_smart_account(ctx: Context<Self>, args: CreateSmartAccountArgs) -> Result<()> {
        let program_config = &mut ctx.accounts.program_config;
        // Sort the members by pubkey.
        let mut signers = args.signers;
        signers.sort_by_key(|m| m.key);

        // Initialize the smart account.
        let settings = &mut ctx.accounts.settings;
        settings.settings_authority = args.settings_authority.unwrap_or_default();
        settings.threshold = args.threshold;
        settings.time_lock = args.time_lock;
        settings.transaction_index = 0;
        settings.stale_transaction_index = 0;
        settings.seed = program_config.smart_account_index.checked_add(1).unwrap();
        settings.bump = ctx.bumps.settings;
        settings.signers = signers;
        // Preset to Pubkey::default() until archival feature is implemented.
        settings.archival_authority = Some(Pubkey::default());

        settings.invariant()?;

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

        Ok(())
    }
}
