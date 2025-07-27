use account_events::SynchronousTransactionEvent;
use anchor_lang::prelude::*;

use crate::{
    consensus::ConsensusAccount,
    consensus_trait::{Consensus, ConsensusAccountType},
    errors::*,
    events::*,
    program::SquadsSmartAccountProgram,
    state::*,
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
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SyncTransactionArgs {
    pub account_index: u8,
    pub num_signers: u8,
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
    // 2. Any remaining accounts associated with the instructions
}

impl<'info> SyncTransaction<'info> {
    fn validate(
        &self,
        args: &SyncTransactionArgs,
        remaining_accounts: &[AccountInfo],
    ) -> Result<()> {
        let Self {
            consensus_account, ..
        } = self;

        // Check that the consensus account is active (policy)
        consensus_account.is_active(remaining_accounts)?;

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

        // Synchronous consensus validation
        validate_synchronous_consensus(&consensus_account, args.num_signers, remaining_accounts)
    }
}

impl<'info> SyncTransaction<'info> {
    #[access_control(ctx.accounts.validate(&args, &ctx.remaining_accounts))]
    pub fn sync_transaction(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        args: SyncTransactionArgs,
    ) -> Result<()> {
        // Readonly Accounts
        let consensus_account = &mut ctx.accounts.consensus_account;
        // Remove the signers from the remaining accounts
        let remaining_accounts = &ctx.remaining_accounts[args.num_signers as usize..];
        let consensus_account_key = consensus_account.key();
        match consensus_account.account_type() {
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
                    settings_compiled_instructions,
                    &remaining_accounts,
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
                        .map(|acc| acc.key.clone())
                        .collect(),
                    account_index: args.account_index,
                    instructions: executable_message.instructions,
                    instruction_accounts: executable_message
                        .accounts
                        .iter()
                        .map(|a| a.key.clone())
                        .collect(),
                };
                let log_authority_info = LogAuthorityInfo {
                    authority: consensus_account.to_account_info(),
                    authority_seeds: get_settings_signer_seeds(settings.seed),
                    bump: settings.bump,
                    program: ctx.accounts.program.to_account_info(),
                };
                SmartAccountEvent::SynchronousTransactionEvent(event).log(&log_authority_info)?;
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

                let remaining_accounts = &ctx.remaining_accounts[account_offset..];
                // Execute the policy
                policy.execute(None, None, payload, &remaining_accounts)?;

            }
        }

        // Check the policy invariant
        consensus_account.invariant()?;

        Ok(())
    }
}

