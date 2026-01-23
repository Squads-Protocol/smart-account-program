use anchor_lang::prelude::*;

use crate::{
    consensus::ConsensusAccount,
    consensus_trait::{Consensus, ConsensusAccountType},
    errors::*,
    events::*,
    program::SquadsSmartAccountProgram,
    state::*,
    utils::{collect_v2_signer_pubkeys, validate_synchronous_consensus_v2, SyncConsensusV2Args, SyncConsensusV2Result, SynchronousTransactionMessage},
    SmallVec,
};

use super::{CompiledInstruction, SyncPayload};

/// Args for V2 synchronous transaction execution with external signer support
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SyncTransactionV2Args {
    pub account_index: u8,
    /// Number of native signers (directly signing the transaction)
    pub num_native_signers: u8,
    /// Key IDs of external signers (verified via precompile)
    pub external_signer_key_ids: Vec<Pubkey>,
    /// Client data params for WebAuthn verification (required if any WebAuthn signers)
    pub client_data_params: Option<ClientDataJsonReconstructionParams>,
    /// The payload to execute
    pub payload: SyncPayload,
}

#[derive(Accounts)]
pub struct SyncTransactionV2<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok(),
    )]
    pub consensus_account: Box<InterfaceAccount<'info, ConsensusAccount>>,
    pub program: Program<'info, SquadsSmartAccountProgram>,
    // `remaining_accounts` must include the following accounts in the exact order:
    // 1. Instructions sysvar (if external signers are used)
    // 2. Native signer accounts (num_native_signers count)
    // 3. For transaction execution:
    //   3.1. Any remaining accounts associated with the instructions
    // 4. For policy execution:
    //   4.1 Settings account if the policy has a settings state expiration
    //   4.2 Any remaining accounts associated with the policy
}

impl<'info> SyncTransactionV2<'info> {
    fn validate(
        &self,
        args: &SyncTransactionV2Args,
        remaining_accounts: &[AccountInfo],
    ) -> Result<SyncConsensusV2Result> {
        let Self {
            consensus_account, ..
        } = self;

        // Calculate how many accounts are used for consensus
        let consensus_args = SyncConsensusV2Args {
            num_native_signers: args.num_native_signers,
            external_signer_key_ids: args.external_signer_key_ids.clone(),
            client_data_params: args.client_data_params,
        };

        // Validate V2 consensus with external signer support
        let consensus_result = validate_synchronous_consensus_v2(
            consensus_account,
            &consensus_args,
            consensus_account.key(),
            remaining_accounts,
        )?;

        // Get remaining accounts after consensus accounts
        let remaining_after_consensus = &remaining_accounts[consensus_result.accounts_consumed..];

        // Check that the consensus account is active (policy)
        consensus_account.is_active(remaining_after_consensus)?;

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
                    policy.validate_payload(PolicyExecutionContext::Synchronous, payload)?;
                }
                _ => {
                    return Err(SmartAccountError::ProgramInteractionAsyncPayloadNotAllowedWithSyncTransaction.into());
                }
            }
        }

        Ok(consensus_result)
    }
}

impl<'info> SyncTransactionV2<'info> {
    pub fn sync_transaction_v2(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncTransactionV2Args,
    ) -> Result<()> {
        // Validate and get consensus result
        let consensus_result = ctx.accounts.validate(&args, &ctx.remaining_accounts)?;

        // Mutable consensus account
        let consensus_account = &mut ctx.accounts.consensus_account;

        // Apply WebAuthn counter updates to prevent replay attacks
        consensus_account.apply_counter_updates(&consensus_result.counter_updates)?;

        // Remove consensus accounts from remaining
        let remaining_accounts = &ctx.remaining_accounts[consensus_result.accounts_consumed..];

        let consensus_account_key = consensus_account.key();

        // Log authority info
        let log_authority_info = LogAuthorityInfo {
            authority: consensus_account.to_account_info(),
            authority_seeds: consensus_account.get_signer_seeds(),
            bump: consensus_account.bump(),
            program: ctx.accounts.program.to_account_info(),
        };

        // Collect signer pubkeys for event (native + external)
        let signer_pubkeys = collect_v2_signer_pubkeys(
            args.num_native_signers,
            &args.external_signer_key_ids,
            &ctx.remaining_accounts,
        );

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
                    &settings.signers,
                    &settings_compiled_instructions,
                    &remaining_accounts,
                )?;

                // Execute the transaction message instructions one-by-one.
                executable_message.execute(smart_account_signer_seeds)?;

                // Create the event
                SynchronousTransactionEventV2 {
                    consensus_account: settings_key,
                    consensus_account_type: ConsensusAccountType::Settings,
                    payload: SynchronousTransactionEventPayload::TransactionPayload {
                        account_index: args.account_index,
                        instructions: executable_message.instructions.to_vec(),
                    },
                    signers: signer_pubkeys,
                    instruction_accounts: executable_message
                        .accounts
                        .iter()
                        .map(|a| *a.key)
                        .collect(),
                }
            }
            ConsensusAccountType::Policy => {
                let payload = args.payload.to_policy_payload()?;
                let policy = consensus_account.policy()?;

                // Determine account offset based on policy expiration type
                let account_offset = policy
                    .expiration
                    .as_ref()
                    .map(|exp| match exp {
                        PolicyExpiration::SettingsState(_) => 1,
                        _ => 0,
                    })
                    .unwrap_or(0);

                // Potentially remove the settings account for expiration
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
                SynchronousTransactionEventV2 {
                    consensus_account: consensus_account_key,
                    consensus_account_type: ConsensusAccountType::Policy,
                    payload: SynchronousTransactionEventPayload::PolicyPayload {
                        policy_payload: payload.clone(),
                    },
                    signers: signer_pubkeys,
                    instruction_accounts: remaining_accounts
                        .iter()
                        .map(|acc| *acc.key)
                        .collect(),
                }
            }
        };

        // Check the policy invariant
        consensus_account.invariant()?;

        SmartAccountEvent::SynchronousTransactionEventV2(event).log(&log_authority_info)?;

        Ok(())
    }
}
