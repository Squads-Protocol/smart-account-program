use anchor_lang::prelude::*;

use crate::{
    consensus::ConsensusAccount,
    consensus_trait::{Consensus, ConsensusAccountType},
    errors::*,
    events::*,
    program::SquadsSmartAccountProgram,
    state::*,
    utils::{
        collect_v2_signer_pubkeys, validate_synchronous_consensus,
        validate_synchronous_consensus_v2, SyncConsensusV2Args, SyncConsensusV2Result,
        SynchronousTransactionMessage,
    },
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
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SyncTransactionArgs {
    pub account_index: u8,
    pub num_signers: u8,
    pub payload: SyncPayload,
}

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
pub struct SyncTransaction<'info> {
    #[account(
        mut,
        constraint = consensus_account.check_derivation(consensus_account.key()).is_ok(),
    )]
    pub consensus_account: Box<InterfaceAccount<'info, ConsensusAccount>>,
    pub program: Program<'info, SquadsSmartAccountProgram>,
    // `remaining_accounts` must include the following accounts in the exact order:
    // 1. The exact amount of signers required to reach the threshold
    // 2. For transaction execution:
    //   2.1. Any remaining accounts associated with the instructions
    // 3. For policy execution:
    //   3.1 Settings account if the policy has a settings state expiration
    //   3.2 Any remaining accounts associated with the policy
}

impl<'info> SyncTransaction<'info> {
    fn validate_account_index(
        consensus_account: &InterfaceAccount<'info, ConsensusAccount>,
        account_index: u8,
    ) -> Result<()> {
        if consensus_account.account_type() == ConsensusAccountType::Settings {
            let settings = consensus_account.read_only_settings()?;
            settings.validate_account_index_unlocked(account_index)?;
        }

        Ok(())
    }

    fn validate_payload(
        consensus_account: &InterfaceAccount<'info, ConsensusAccount>,
        payload: &SyncPayload,
    ) -> Result<()> {
        if consensus_account.account_type() == ConsensusAccountType::Policy {
            let policy = consensus_account.read_only_policy()?;
            match payload {
                SyncPayload::Policy(payload) => {
                    policy.validate_payload(PolicyExecutionContext::Synchronous, payload)?;
                }
                _ => {
                    return Err(
                        SmartAccountError::ProgramInteractionAsyncPayloadNotAllowedWithSyncTransaction
                            .into(),
                    );
                }
            }
        }

        Ok(())
    }

    fn validate(
        &self,
        args: &SyncTransactionArgs,
        remaining_accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        let Self {
            consensus_account, ..
        } = self;

        // Check that the consensus account is active (policy)
        consensus_account.is_active(&remaining_accounts[args.num_signers as usize..])?;

        // Validate account index is unlocked for Settings-based transactions
        Self::validate_account_index(consensus_account, args.account_index)?;
        Self::validate_payload(consensus_account, &args.payload)?;

        validate_synchronous_consensus(&consensus_account, args.num_signers, remaining_accounts)
    }

    fn validate_v2(
        &self,
        args: &SyncTransactionV2Args,
        remaining_accounts: &'info [AccountInfo<'info>],
    ) -> Result<SyncConsensusV2Result> {
        let Self {
            consensus_account, ..
        } = self;

        let consensus_args = SyncConsensusV2Args {
            num_native_signers: args.num_native_signers,
            external_signer_key_ids: args.external_signer_key_ids.clone(),
            client_data_params: args.client_data_params,
        };

        let consensus_result = validate_synchronous_consensus_v2(
            consensus_account,
            &consensus_args,
            consensus_account.key(),
            remaining_accounts,
        )?;

        let remaining_after_consensus = &remaining_accounts[consensus_result.accounts_consumed..];

        consensus_account.is_active(remaining_after_consensus)?;
        Self::validate_account_index(consensus_account, args.account_index)?;
        Self::validate_payload(consensus_account, &args.payload)?;

        Ok(consensus_result)
    }
}

impl<'info> SyncTransaction<'info> {
    fn execute_inner(
        consensus_account: &mut Box<InterfaceAccount<'info, ConsensusAccount>>,
        account_index: u8,
        payload: SyncPayload,
        remaining_accounts: &'info [AccountInfo<'info>],
        signer_pubkeys: Vec<Pubkey>,
        program: &Program<'info, SquadsSmartAccountProgram>,
        program_id: &Pubkey,
    ) -> Result<()> {
        let consensus_account_key = consensus_account.key();

        let log_authority_info = LogAuthorityInfo {
            authority: consensus_account.to_account_info(),
            authority_seeds: consensus_account.get_signer_seeds(),
            bump: consensus_account.bump(),
            program: program.to_account_info(),
        };

        let event = match consensus_account.account_type() {
            ConsensusAccountType::Settings => {
                let payload = payload.to_transaction_payload()?;

                let settings = consensus_account.read_only_settings()?;
                let settings_key = consensus_account_key;
                let compiled_instructions =
                    SmallVec::<u8, CompiledInstruction>::try_from_slice(&payload)
                        .map_err(|_| SmartAccountError::InvalidInstructionArgs)?;
                let settings_compiled_instructions: Vec<SmartAccountCompiledInstruction> =
                    Vec::from(compiled_instructions)
                        .into_iter()
                        .map(SmartAccountCompiledInstruction::from)
                        .collect();

                let smart_account_seeds = &[
                    SEED_PREFIX,
                    settings_key.as_ref(),
                    SEED_SMART_ACCOUNT,
                    &account_index.to_le_bytes(),
                ];

                let (smart_account_pubkey, smart_account_bump) =
                    Pubkey::find_program_address(smart_account_seeds, program_id);

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
                    remaining_accounts,
                )?;

                executable_message.execute(smart_account_signer_seeds)?;

                SynchronousTransactionEventV2 {
                    consensus_account: settings_key,
                    consensus_account_type: ConsensusAccountType::Settings,
                    payload: SynchronousTransactionEventPayload::TransactionPayload {
                        account_index,
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
                let payload = payload.to_policy_payload()?;
                let policy = consensus_account.policy()?;

                let account_offset = policy
                    .expiration
                    .as_ref()
                    .map(|exp| match exp {
                        PolicyExpiration::SettingsState(_) => 1,
                        _ => 0,
                    })
                    .unwrap_or(0);

                let remaining_accounts = &remaining_accounts[account_offset..];

                policy.execute(None, None, payload, &remaining_accounts)?;

                let policy_update_event = PolicyEvent {
                    event_type: PolicyEventType::UpdateDuringExecution,
                    settings_pubkey: policy.settings,
                    policy_pubkey: consensus_account_key,
                    policy: Some(policy.clone()),
                };

                SmartAccountEvent::PolicyEvent(policy_update_event).log(&log_authority_info)?;

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

        consensus_account.invariant()?;

        SmartAccountEvent::SynchronousTransactionEventV2(event).log(&log_authority_info)?;

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts))]
    pub fn sync_transaction(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncTransactionArgs,
    ) -> Result<()> {
        let consensus_account = &mut ctx.accounts.consensus_account;
        let remaining_accounts = &ctx.remaining_accounts[args.num_signers as usize..];
        let signer_pubkeys = ctx.remaining_accounts[..args.num_signers as usize]
            .iter()
            .map(|acc| *acc.key)
            .collect::<Vec<_>>();

        Self::execute_inner(
            consensus_account,
            args.account_index,
            args.payload,
            remaining_accounts,
            signer_pubkeys,
            &ctx.accounts.program,
            ctx.program_id,
        )
    }

    pub fn sync_transaction_v2(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncTransactionV2Args,
    ) -> Result<()> {
        let consensus_result = ctx.accounts.validate_v2(&args, &ctx.remaining_accounts)?;
        let consensus_account = &mut ctx.accounts.consensus_account;

        consensus_account.apply_counter_updates(&consensus_result.counter_updates)?;

        let remaining_accounts = &ctx.remaining_accounts[consensus_result.accounts_consumed..];
        let signer_pubkeys = collect_v2_signer_pubkeys(
            args.num_native_signers,
            &args.external_signer_key_ids,
            &ctx.remaining_accounts,
        );

        Self::execute_inner(
            consensus_account,
            args.account_index,
            args.payload,
            remaining_accounts,
            signer_pubkeys,
            &ctx.accounts.program,
            ctx.program_id,
        )
    }
}
