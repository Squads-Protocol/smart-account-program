use account_events::SynchronousTransactionEvent;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash;

use crate::{
    consensus::ConsensusAccount,
    consensus_trait::{Consensus, ConsensusAccountType},
    errors::*,
    events::*,
    program::SquadsSmartAccountProgram,
    state::*,
    state::signer_v2::ExtraVerificationData,
    state::signer_v2::precompile::create_sync_consensus_message,
    utils::{validate_synchronous_consensus, SynchronousTransactionMessage},
    SmallVec,
};

use super::CompiledInstruction;

/// Arguments for synchronous transaction execution (legacy)
///
/// # BREAKING CHANGE (v2)
/// `num_signers` now represents the TOTAL count of ALL signers (native + external).
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct LegacySyncTransactionArgs {
    /// The index of the smart account this transaction is for
    pub account_index: u8,
    /// Total count of ALL signers (native + external) in remaining_accounts.
    /// Instructions sysvar must be at position num_signers if external signers present.
    pub num_signers: u8,
    /// Expected to be serialized as a SmallVec<u8, CompiledInstruction>
    pub instructions: Vec<u8>,
}

/// Account structure for legacy synchronous transaction execution
///
/// # Remaining Accounts (BREAKING CHANGE v2)
/// ```
/// [0..num_signers]      All signers (native + external)
/// [num_signers]         Instructions sysvar (if external signers present)
/// [num_signers+1..]     Transaction account addresses
/// ```
#[derive(Accounts)]
pub struct LegacySyncTransaction<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok(),
        // Legacy sync transactions only support settings
        constraint = consensus_account.account_type() == ConsensusAccountType::Settings
    )]
    pub consensus_account: Box<InterfaceAccount<'info, ConsensusAccount>>,
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl LegacySyncTransaction<'_> {
    fn validate(
        &mut self,
        args: &LegacySyncTransactionArgs,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<SmallVec<u8, ExtraVerificationData>>,
    ) -> Result<()> {
        let Self { consensus_account, .. } = self;

        // Validate account index is unlocked
        let settings = consensus_account.read_only_settings()?;
        settings.validate_account_index_unlocked(args.account_index)?;

        // Build message for external signer verification.
        // Hash the instructions payload so external signers commit to the exact instructions.
        let payload_hash = hash(&args.instructions);
        let message = create_sync_consensus_message(
            &consensus_account.key(),
            consensus_account.transaction_index(),
            &payload_hash.to_bytes(),
        );

        let evd: &[ExtraVerificationData] = match &extra_verification_data {
            Some(v) => v,
            None => &[],
        };
        validate_synchronous_consensus(consensus_account, args.num_signers, remaining_accounts, message, evd)
    }
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, None))]
    pub fn sync_transaction(ctx: Context<Self>, args: LegacySyncTransactionArgs) -> Result<()> {
        Self::sync_transaction_inner(ctx, args)
    }

    /// Legacy sync transaction with V2 signer support.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, extra_verification_data))]
    pub fn sync_transaction_v2(ctx: Context<Self>, args: LegacySyncTransactionArgs, extra_verification_data: Option<SmallVec<u8, ExtraVerificationData>>) -> Result<()> {
        Self::sync_transaction_inner(ctx, args)
    }

    fn sync_transaction_inner(ctx: Context<Self>, args: LegacySyncTransactionArgs) -> Result<()> {
        // Wrapper consensus account
        let consensus_account = &ctx.accounts.consensus_account;
        let settings = consensus_account.read_only_settings()?;
        let settings_key = consensus_account.key();
        let settings_account_info = consensus_account.to_account_info();

        // Deserialize the instructions
        let compiled_instructions =
            SmallVec::<u8, CompiledInstruction>::try_from_slice(&args.instructions)
                .map_err(|_| SmartAccountError::InvalidInstructionArgs)?;
        // Convert to SmartAccountCompiledInstruction
        let settings_compiled_instructions: Vec<SmartAccountCompiledInstruction> =
            Vec::from(compiled_instructions)
                .into_iter()
                .map(SmartAccountCompiledInstruction::from)
                .collect();

        let smart_account_seeds = &[
            SEED_PREFIX,
            settings_key.as_ref(),
            SEED_SMART_ACCOUNT,
            &args.account_index.to_le_bytes(),
        ];

        let (smart_account_pubkey, smart_account_bump) =
            Pubkey::find_program_address(smart_account_seeds, ctx.program_id);

        // Get the signer seeds for the smart account
        let smart_account_signer_seeds = &[
            smart_account_seeds[0],
            smart_account_seeds[1],
            smart_account_seeds[2],
            smart_account_seeds[3],
            &[smart_account_bump],
        ];

        let executable_message = SynchronousTransactionMessage::new_validated(
            &settings_key,
            &smart_account_pubkey,
            &settings.signers.as_v2(),
            &settings_compiled_instructions,
            &ctx.remaining_accounts,
        )?;

        // Execute the transaction message instructions one-by-one.
        // NOTE: `execute_message()` calls `self.to_instructions_and_accounts()`
        // which in turn calls `take()` on
        // `self.message.instructions`, therefore after this point no more
        // references or usages of `self.message` should be made to avoid
        // faulty behavior.
        executable_message.execute(smart_account_signer_seeds)?;

        // Log the event
        let event = SynchronousTransactionEvent {
            settings_pubkey: settings_key,
            signers: ctx.remaining_accounts[..args.num_signers as usize]
                .iter()
                .map(|acc| settings.resolve_canonical_key(*acc.key, acc.is_signer))
                .collect::<Result<Vec<_>>>()?,
            account_index: args.account_index,
            instructions: executable_message.instructions.to_vec(),
            instruction_accounts: executable_message
                .accounts
                .iter()
                .map(|a| a.key.clone())
                .collect(),
        };
        let log_authority_info = LogAuthorityInfo {
            authority: settings_account_info,
            authority_seeds: get_settings_signer_seeds(settings.seed),
            bump: settings.bump,
            program: ctx.accounts.program.to_account_info(),
        };
        SmartAccountEvent::SynchronousTransactionEvent(event).log(&log_authority_info)?;
        Ok(())
    }
}
