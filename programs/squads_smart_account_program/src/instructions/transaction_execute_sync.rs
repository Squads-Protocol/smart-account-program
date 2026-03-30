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

#[derive(AnchorSerialize, AnchorDeserialize)]
pub enum SyncPayload {
    Transaction(Vec<u8>),
    Policy(PolicyPayload),
}

impl SyncPayload {
    pub fn to_transaction_payload(&self) -> Result<&Vec<u8>> {
        match self {
            SyncPayload::Transaction(payload) => Ok(payload),
            _ => err!(SmartAccountError::InvalidPayload),
        }
    }

    pub fn to_policy_payload(&self) -> Result<&PolicyPayload> {
        match self {
            SyncPayload::Policy(payload) => Ok(payload),
            _ => err!(SmartAccountError::InvalidPayload),
        }
    }
}
/// Arguments for synchronous transaction execution
///
/// # BREAKING CHANGE (v2)
/// `num_signers` now represents the TOTAL count of ALL signers (native + external),
/// not just native signers. The instructions sysvar (if external signers are present)
/// must be placed at position `num_signers` in remaining_accounts.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SyncTransactionArgs {
    pub account_index: u8,
    /// Total count of ALL signers (native + external) in remaining_accounts.
    /// - Native signers have AccountInfo.is_signer = true
    /// - External signers have AccountInfo.is_signer = false
    /// - Instructions sysvar must be at position num_signers
    pub num_signers: u8,
    pub payload: SyncPayload,
}

/// Account structure for synchronous transaction execution
///
/// # Remaining Accounts (BREAKING CHANGE v2)
/// The order has changed to support unified signer validation:
///
/// ```
/// [0..num_signers]           All signers (native + external mixed)
///                            - Native: AccountInfo.is_signer = true
///                            - External: AccountInfo.is_signer = false
/// [num_signers]              Instructions sysvar (if external signers present)
/// [num_signers+1..]          Transaction or policy-specific accounts
/// ```
///
/// For transaction execution:
/// - Transaction account addresses
///
/// For policy execution:
/// - Settings account (if policy has SettingsState expiration)
/// - Policy-specific accounts
#[derive(Accounts)]
pub struct SyncTransaction<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok(),
    )]
    pub consensus_account: Box<InterfaceAccount<'info, ConsensusAccount>>,
    pub program: Program<'info, SquadsSmartAccountProgram>,
}

impl<'info> SyncTransaction<'info> {
    fn validate(
        &mut self,
        args: &SyncTransactionArgs,
        remaining_accounts: &[AccountInfo],
        extra_verification_data: Option<SmallVec<u8, ExtraVerificationData>>,
    ) -> Result<()> {
        let Self {
            consensus_account, ..
        } = self;

        // Compute the offset past signers + optional instructions sysvar.
        // When external signers are present, the instructions sysvar sits at
        // remaining_accounts[num_signers]. We must skip it before passing
        // accounts to is_active() and downstream execution.
        let sysvar_offset = if remaining_accounts
            .get(args.num_signers as usize)
            .map_or(false, |acc| acc.key == &anchor_lang::solana_program::sysvar::instructions::ID)
        { 1usize } else { 0usize };
        let accounts_start = args.num_signers as usize + sysvar_offset;

        // Check that the consensus account is active (policy)
        consensus_account.is_active(&remaining_accounts[accounts_start..])?;

        // Validate account index is unlocked for Settings-based transactions
        if consensus_account.account_type() == ConsensusAccountType::Settings {
            let settings = consensus_account.read_only_settings()?;
            settings.validate_account_index_unlocked(args.account_index)?;
        }

        // Validate policy payload if necessary
        if consensus_account.account_type() == ConsensusAccountType::Policy {
            let policy = consensus_account.read_only_policy()?;
            match &args.payload {
                SyncPayload::Policy(payload) => {
                    // Validate the payload against the policy state
                    policy.validate_payload(PolicyExecutionContext::Synchronous, payload)?;
                }
                _ => {
                    return Err(SmartAccountError::ProgramInteractionAsyncPayloadNotAllowedWithSyncTransaction.into());
                }
            }
        }

        // Build message for external signer verification.
        // Hash the payload so external signers commit to the exact instructions being executed.
        let payload_bytes = args.payload.try_to_vec()
            .map_err(|_| SmartAccountError::InvalidPayload)?;
        let payload_hash = hash(&payload_bytes);
        let message = create_sync_consensus_message(
            &consensus_account.key(),
            consensus_account.transaction_index(),
            &payload_hash.to_bytes(),
        );

        // Synchronous consensus validation
        let evd: &[ExtraVerificationData] = match &extra_verification_data {
            Some(v) => v,
            None => &[],
        };
        validate_synchronous_consensus(consensus_account, args.num_signers, remaining_accounts, message, evd)
    }
}

impl<'info> SyncTransaction<'info> {
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, None))]
    pub fn sync_transaction(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncTransactionArgs,
    ) -> Result<()> {
        Self::sync_transaction_inner(ctx, args)
    }

    /// Sync transaction with V2 signer support.
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts, extra_verification_data))]
    pub fn sync_transaction_v2(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncTransactionArgs,
        extra_verification_data: Option<SmallVec<u8, ExtraVerificationData>>,
    ) -> Result<()> {
        Self::sync_transaction_inner(ctx, args)
    }

    fn sync_transaction_inner(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncTransactionArgs,
    ) -> Result<()> {
        // Readonly Accounts
        let consensus_account = &mut ctx.accounts.consensus_account;
        // Remove the signers (and optional instructions sysvar) from the remaining accounts.
        // When external signers are present, the sysvar sits at remaining_accounts[num_signers].
        let sysvar_offset = if ctx.remaining_accounts
            .get(args.num_signers as usize)
            .map_or(false, |acc| acc.key == &anchor_lang::solana_program::sysvar::instructions::ID)
        { 1usize } else { 0usize };
        let remaining_accounts = &ctx.remaining_accounts[args.num_signers as usize + sysvar_offset..];

        let consensus_account_key = consensus_account.key();

        // Log authority info
        let log_authority_info = LogAuthorityInfo {
            authority: consensus_account.to_account_info(),
            authority_seeds: consensus_account.get_signer_seeds(),
            bump: consensus_account.bump(),
            program: ctx.accounts.program.to_account_info(),
        };
        let event = match consensus_account.account_type() {
            ConsensusAccountType::Settings => {
                // Get the payload
                let payload = args.payload.to_transaction_payload()?;

                let settings = consensus_account.read_only_settings()?;
                let settings_key = consensus_account_key;
                // Deserialize the instructions
                let compiled_instructions =
                    SmallVec::<u8, CompiledInstruction>::try_from_slice(&payload)
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
                    &remaining_accounts,
                )?;

                // Execute the transaction message instructions one-by-one.
                // NOTE: `execute_message()` calls `self.to_instructions_and_accounts()`
                // which in turn calls `take()` on
                // `self.message.instructions`, therefore after this point no more
                // references or usages of `self.message` should be made to avoid
                // faulty behavior.
                executable_message.execute(smart_account_signer_seeds)?;

                // Create the event
                let event = SynchronousTransactionEventV2 {
                    consensus_account: settings_key,
                    consensus_account_type: ConsensusAccountType::Settings,
                    payload: SynchronousTransactionEventPayload::TransactionPayload {
                        account_index: args.account_index,
                        instructions: executable_message.instructions.to_vec(),
                    },
                    signers: ctx.remaining_accounts[..args.num_signers as usize]
                        .iter()
                        .map(|acc| consensus_account.resolve_canonical_key(*acc.key, acc.is_signer))
                        .collect::<Result<Vec<_>>>()?,
                    instruction_accounts: executable_message
                        .accounts
                        .iter()
                        .map(|a| a.key.clone())
                        .collect(),
                };
                event
            }
            ConsensusAccountType::Policy => {
                let payload = args.payload.to_policy_payload()?;
                let policy = consensus_account.policy()?;

                // Determine account offset based on policy expiration type
                let account_offset = policy
                    .expiration
                    .as_ref()
                    .map(|exp| match exp {
                        // The settings is the first extra remaining account
                        PolicyExpiration::SettingsState(_) => 1,
                        _ => 0,
                    })
                    .unwrap_or(0);

                // Potentially remove the settings account for expiration from
                // the remaining accounts
                let remaining_accounts = &remaining_accounts[account_offset..];

                // Execute the policy
                policy.execute(None, None, payload, &remaining_accounts)?;

                // Policy may updated during execution, log the event
                let policy_update_event = PolicyEvent {
                    event_type: PolicyEventType::UpdateDuringExecution,
                    settings_pubkey: policy.settings,
                    policy_pubkey: consensus_account_key,
                    policy: Some(policy.clone()),
                };

                SmartAccountEvent::PolicyEvent(policy_update_event).log(&log_authority_info)?;

                // Create the event
                let event = SynchronousTransactionEventV2 {
                    consensus_account: consensus_account_key,
                    consensus_account_type: ConsensusAccountType::Policy,
                    payload: SynchronousTransactionEventPayload::PolicyPayload {
                        policy_payload: payload.clone(),
                    },
                    signers: ctx.remaining_accounts[..args.num_signers as usize]
                        .iter()
                        .map(|acc| consensus_account.resolve_canonical_key(*acc.key, acc.is_signer))
                        .collect::<Result<Vec<_>>>()?,
                    instruction_accounts: remaining_accounts
                        .iter()
                        .map(|acc| acc.key.clone())
                        .collect(),
                };
                event
            }
        };

        // Check the policy invariant
        consensus_account.invariant()?;

        SmartAccountEvent::SynchronousTransactionEventV2(event).log(&log_authority_info)?;

        Ok(())
    }
}
