//! `ProgramInteractionPolicy` — type definitions, constraints, payload types,
//! and pure-data methods (`Hook::size`, `DataConstraint::evaluate`,
//! `AccountConstraint::size`, sizes, etc.) live in the types crate.
//!
//! Program-local pieces:
//!   * `PolicyTrait` impl (dispatches async vs sync execution)
//!   * `Hook::execute`, `AccountConstraint::evaluate_against_account_info*`
//!     (CPI + AccountInfo operations)
//!   * `ProgramInteractionPolicyExt::validate_payload` (anchor Result dispatch)
//!   * `program_interaction_creation_to_policy_state` (Clock-based helper)

use anchor_lang::prelude::*;
use solana_program::instruction::Instruction;

pub use squads_smart_account_program_types::{
    AccountConstraint, AccountConstraintType, DataConstraint, DataOperator, DataValue, Hook,
    InstructionConstraint, LimitedQuantityConstraints, LimitedSpendingLimit,
    LimitedTimeConstraints, ProgramInteractionExecutionArgs, ProgramInteractionPayload,
    ProgramInteractionPolicy, ProgramInteractionPolicyCreationPayload,
    ProgramInteractionTransactionPayload, SyncTransactionPayloadDetails,
};
use squads_smart_account_program_types::{
    QuantityConstraints, SpendingLimitV2, TimeConstraints, UsageState,
};

use crate::error_conv::ToAnchorResult;
use crate::{
    errors::*,
    state::policies::{
        policy_core::{PolicyExecutionContext, PolicyTrait},
        utils::check_pre_balances,
    },
    utils::{
        derive_ephemeral_signers, ExecutableTransactionMessage, SynchronousTransactionMessage,
    },
    CompiledInstruction, SmallVec, SmartAccountCompiledInstruction, TransactionMessage,
    HOOK_AUTHORITY_PUBKEY, SEED_EPHEMERAL_SIGNER, SEED_HOOK_AUTHORITY, SEED_PREFIX,
    SEED_SMART_ACCOUNT,
};

// =============================================================================
// CREATION CONVERSION (Clock-dependent)
// =============================================================================

/// Convert a `ProgramInteractionPolicyCreationPayload` into the runtime
/// policy state, resolving start timestamps against the current Clock.
pub fn program_interaction_creation_to_policy_state(
    payload: ProgramInteractionPolicyCreationPayload,
) -> Result<ProgramInteractionPolicy> {
    require!(
        payload.instructions_constraints.len() <= 20,
        SmartAccountError::ProgramInteractionTooManyInstructionConstraints
    );
    require!(
        payload.spending_limits.len() <= 10,
        SmartAccountError::ProgramInteractionTooManySpendingLimits
    );

    let mut spending_limits = payload.spending_limits.clone();
    spending_limits.sort_by_key(|c| c.mint);

    let current_timestamp = Clock::get()?.unix_timestamp;
    Ok(ProgramInteractionPolicy {
        account_index: payload.account_index,
        instructions_constraints: payload.instructions_constraints,
        pre_hook: payload.pre_hook,
        post_hook: payload.post_hook,
        spending_limits: spending_limits
            .iter()
            .map(|spending_limit| {
                let start = if spending_limit.time_constraints.start == 0 {
                    current_timestamp
                } else {
                    spending_limit.time_constraints.start
                };
                SpendingLimitV2 {
                    mint: spending_limit.mint,
                    time_constraints: TimeConstraints {
                        start,
                        period: spending_limit.time_constraints.period,
                        expiration: spending_limit.time_constraints.expiration,
                        accumulate_unused: false,
                    },
                    quantity_constraints: QuantityConstraints {
                        max_per_period: spending_limit.quantity_constraints.max_per_period,
                        max_per_use: 0,
                        enforce_exact_quantity: false,
                    },
                    usage: UsageState {
                        remaining_in_period: spending_limit.quantity_constraints.max_per_period,
                        last_reset: start,
                    },
                }
            })
            .collect(),
    })
}

// =============================================================================
// EXTENSION TRAITS
// =============================================================================

pub trait ProgramInteractionPolicyExt {
    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &ProgramInteractionPayload,
    ) -> Result<()>;

    fn evaluate_instruction_constraints<'info>(
        &self,
        instruction_constraint_indices: &[u8],
        instructions: &[SmartAccountCompiledInstruction],
        accounts: &[AccountInfo<'info>],
    ) -> Result<()>;

    fn parse_hook_accounts<'info, 'a>(
        &self,
        accounts: &mut &'a [AccountInfo<'info>],
    ) -> (&'a [AccountInfo<'info>], &'a [AccountInfo<'info>]);
}

impl ProgramInteractionPolicyExt for ProgramInteractionPolicy {
    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &ProgramInteractionPayload,
    ) -> Result<()> {
        match (context, &payload.transaction_payload) {
            (
                PolicyExecutionContext::Synchronous,
                ProgramInteractionTransactionPayload::AsyncTransaction(..),
            ) => {
                return Err(
                    SmartAccountError::ProgramInteractionAsyncPayloadNotAllowedWithSyncTransaction
                        .into(),
                );
            }
            (
                PolicyExecutionContext::Asynchronous,
                ProgramInteractionTransactionPayload::SyncTransaction(..),
            ) => {
                return Err(
                    SmartAccountError::ProgramInteractionSyncPayloadNotAllowedWithAsyncTransaction
                        .into(),
                );
            }
            (_, _) => {}
        }

        let payload_account_index = payload.transaction_payload.get_account_index();
        let instructions_len = match &payload.transaction_payload {
            ProgramInteractionTransactionPayload::AsyncTransaction(transaction_payload) => {
                TransactionMessage::deserialize(
                    &mut transaction_payload.transaction_message.as_slice(),
                )?
                .instructions
                .len()
            }
            ProgramInteractionTransactionPayload::SyncTransaction(sync_transaction_payload) => {
                let instructions: SmallVec<u8, CompiledInstruction> =
                    SmallVec::<u8, CompiledInstruction>::try_from_slice(
                        &sync_transaction_payload.instructions,
                    )
                    .map_err(|_| SmartAccountError::InvalidInstructionArgs)?;
                instructions.len()
            }
        };
        require_eq!(
            payload_account_index,
            self.account_index,
            SmartAccountError::InvalidPayload
        );

        if !self.instructions_constraints.is_empty() {
            if let Some(instruction_constraint_indices) = &payload.instruction_constraint_indices {
                require_eq!(
                    instruction_constraint_indices.len(),
                    instructions_len,
                    SmartAccountError::ProgramInteractionInstructionCountMismatch
                );
                for instruction_constraint_index in instruction_constraint_indices {
                    require!(
                        *instruction_constraint_index < self.instructions_constraints.len() as u8,
                        SmartAccountError::ProgramInteractionConstraintIndexOutOfBounds
                    );
                }
            } else {
                return Err(SmartAccountError::ProgramInteractionInstructionCountMismatch.into());
            }
        }
        Ok(())
    }

    /// Evaluate the account constraint for a given set of instruction_account_indices and accounts
    fn evaluate_instruction_constraints<'info>(
        &self,
        instruction_constraint_indices: &[u8],
        instructions: &[SmartAccountCompiledInstruction],
        accounts: &[AccountInfo<'info>],
    ) -> Result<()> {
        for (instruction, instruction_constraint_index) in
            instructions.iter().zip(instruction_constraint_indices)
        {
            let instruction_constraint =
                &self.instructions_constraints[*instruction_constraint_index as usize];
            require!(
                accounts[instruction.program_id_index as usize].key
                    == &instruction_constraint.program_id,
                SmartAccountError::ProgramInteractionProgramIdMismatch
            );

            for account_constraint in &instruction_constraint.account_constraints {
                evaluate_ac_against_instruction_indices_and_accounts(
                    account_constraint,
                    &instruction.account_indexes,
                    accounts,
                )?;
            }
            for data_constraint in &instruction_constraint.data_constraints {
                data_constraint
                    .evaluate(instruction.data.as_slice())
                    .to_anchor()?;
            }
        }
        Ok(())
    }

    /// Evaluate the account constraint against a set of AccountInfos
    fn parse_hook_accounts<'info, 'a>(
        &self,
        accounts: &mut &'a [AccountInfo<'info>],
    ) -> (&'a [AccountInfo<'info>], &'a [AccountInfo<'info>]) {
        let mut pre_hook_accounts_intermediate: &[AccountInfo<'info>] = &[];
        let mut post_hook_accounts_intermediate: &[AccountInfo<'info>] = &[];
        let mut transaction_accounts = *accounts;

        if self.pre_hook.is_some() {
            let (pre_hook_accounts, remaining_accounts) =
                transaction_accounts.split_at(self.pre_hook.as_ref().unwrap().num_accounts());
            pre_hook_accounts_intermediate = pre_hook_accounts;
            transaction_accounts = remaining_accounts;
        };
        if self.post_hook.is_some() {
            let (post_hook_accounts, remaining_accounts) =
                transaction_accounts.split_at(self.post_hook.as_ref().unwrap().num_accounts());
            post_hook_accounts_intermediate = post_hook_accounts;
            transaction_accounts = remaining_accounts;
        }

        *accounts = transaction_accounts;

        (
            pre_hook_accounts_intermediate,
            post_hook_accounts_intermediate,
        )
    }
}

// =============================================================================
// PAYLOAD HELPERS (AccountInfo/Pubkey plumbing)
// =============================================================================

pub trait ProgramInteractionPayloadExt {
    fn get_transaction_payload(
        &self,
        transaction_key: Pubkey,
    ) -> Result<squads_smart_account_program_types::TransactionPayloadDetails>;

    fn get_sync_transaction_payload(&self) -> Result<&SyncTransactionPayloadDetails>;
}

impl ProgramInteractionPayloadExt for ProgramInteractionPayload {
    fn get_transaction_payload(
        &self,
        transaction_key: Pubkey,
    ) -> Result<squads_smart_account_program_types::TransactionPayloadDetails> {
        use squads_smart_account_program_types::TransactionPayloadDetails;
        match &self.transaction_payload {
            ProgramInteractionTransactionPayload::AsyncTransaction(transaction_payload) => {
                let transaction_message = TransactionMessage::deserialize(
                    &mut transaction_payload.transaction_message.as_slice(),
                )?;
                let ephemeral_signer_bumps: Vec<u8> = (0..transaction_payload.ephemeral_signers)
                    .map(|ephemeral_signer_index| {
                        let ephemeral_signer_seeds = &[
                            SEED_PREFIX,
                            transaction_key.as_ref(),
                            SEED_EPHEMERAL_SIGNER,
                            &ephemeral_signer_index.to_le_bytes(),
                        ];

                        let (_, bump) =
                            Pubkey::find_program_address(ephemeral_signer_seeds, &crate::ID);
                        bump
                    })
                    .collect();

                Ok(TransactionPayloadDetails {
                    account_index: transaction_payload.account_index,
                    ephemeral_signer_bumps,
                    message: transaction_message.try_into()?,
                })
            }
            _ => Err(SmartAccountError::InvalidPayload.into()),
        }
    }

    fn get_sync_transaction_payload(&self) -> Result<&SyncTransactionPayloadDetails> {
        match &self.transaction_payload {
            ProgramInteractionTransactionPayload::SyncTransaction(sync_transaction_payload) => {
                Ok(sync_transaction_payload)
            }
            _ => Err(SmartAccountError::InvalidPayload.into()),
        }
    }
}

// =============================================================================
// HOOK EXECUTION
// =============================================================================

pub trait HookExt {
    fn execute<'info>(
        &self,
        hook_accounts: &'info [AccountInfo<'info>],
        instructions: &[SmartAccountCompiledInstruction],
        instruction_accounts: &[AccountInfo<'info>],
    ) -> Result<()>;
}

impl HookExt for Hook {
    fn execute<'info>(
        &self,
        hook_accounts: &'info [AccountInfo<'info>],
        instructions: &[SmartAccountCompiledInstruction],
        instruction_accounts: &[AccountInfo<'info>],
    ) -> Result<()> {
        use borsh::BorshSerialize;

        // Evaluate the hook accounts
        for account_constraint in self.account_constraints.iter() {
            evaluate_ac_against_account_infos(account_constraint, hook_accounts)?;
        }

        // Build the necessary account metas
        let mut account_metas =
            Vec::with_capacity(1 + hook_accounts.len() + instruction_accounts.len());

        // Add the hook accounts to the account metas
        for account in hook_accounts.iter() {
            let meta = if account.key == &HOOK_AUTHORITY_PUBKEY {
                AccountMeta::new_readonly(*account.key, true)
            } else if account.is_writable {
                AccountMeta::new(*account.key, account.is_signer)
            } else {
                AccountMeta::new_readonly(*account.key, account.is_signer)
            };
            account_metas.push(meta);
        }

        // Build the instruction data
        let mut instruction_data = self.instruction_data.clone();

        if self.pass_inner_instructions {
            // Serialized Vec represenation of the instructions length
            instruction_data.extend_from_slice(&(instructions.len() as u32).to_le_bytes());

            // Serialize the instructions
            for ix in instructions {
                ix.serialize(&mut instruction_data)
                    .map_err(|_| SmartAccountError::ProgramInteractionTemplateHookError)?;
            }

            // Add the instruction accounts
            for account in instruction_accounts.iter() {
                // Allow the hook authority, as long as it is readonly
                let meta = if account.key == &HOOK_AUTHORITY_PUBKEY {
                    AccountMeta::new_readonly(*account.key, true)
                } else if account.is_writable {
                    AccountMeta::new(*account.key, account.is_signer)
                } else {
                    AccountMeta::new_readonly(*account.key, account.is_signer)
                };
                account_metas.push(meta);
            }
        }

        // Build the instruction
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: account_metas,
            data: instruction_data,
        };

        // Invoke the instruction
        anchor_lang::solana_program::program::invoke_signed(
            &instruction,
            // Concatenate the hook accounts and the instruction accounts
            &[hook_accounts, instruction_accounts].concat(),
            &[&[SEED_HOOK_AUTHORITY]],
        )?;
        Ok(())
    }
}

// =============================================================================
// ACCOUNT CONSTRAINT EVALUATION
// =============================================================================

fn evaluate_account_info(constraint: &AccountConstraint, account: &AccountInfo) -> Result<()> {
    if let Some(owner) = constraint.owner {
        require_eq!(
            account.owner,
            &owner,
            SmartAccountError::IllegalAccountOwner
        );
    };
    match &constraint.account_constraint {
        AccountConstraintType::Pubkey(keys) => {
            if !keys.contains(&account.key) {
                return Err(SmartAccountError::ProgramInteractionAccountConstraintViolated.into());
            }
        }
        AccountConstraintType::AccountData(constraints) => {
            let data = account.try_borrow_data()?;
            for c in constraints {
                c.evaluate(&data).to_anchor()?;
            }
        }
    }
    Ok(())
}

fn evaluate_ac_against_instruction_indices_and_accounts(
    constraint: &AccountConstraint,
    instruction_account_indices: &[u8],
    accounts: &[AccountInfo],
) -> Result<()> {
    let mapped_account_index = instruction_account_indices[constraint.account_index as usize];
    let account = &accounts[mapped_account_index as usize];
    evaluate_account_info(constraint, account)?;
    Ok(())
}

fn evaluate_ac_against_account_infos<'info>(
    constraint: &AccountConstraint,
    account_infos: &'info [AccountInfo<'info>],
) -> Result<()> {
    let account_info_to_evaluate = account_infos
        .get(constraint.account_index as usize)
        .ok_or(SmartAccountError::ProgramInteractionAccountConstraintViolated)
        .unwrap();
    evaluate_account_info(constraint, account_info_to_evaluate)?;
    Ok(())
}

// =============================================================================
// POLICY TRAIT IMPL
// =============================================================================

impl PolicyTrait for ProgramInteractionPolicy {
    type PolicyState = Self;
    type CreationPayload = ProgramInteractionPolicyCreationPayload;
    type UsagePayload = ProgramInteractionPayload;
    type ExecutionArgs = ProgramInteractionExecutionArgs;

    /// Validate the policy invariant
    fn invariant(&self) -> Result<()> {
        ProgramInteractionPolicy::invariant(self).to_anchor()
    }

    /// Validate the payload for the policy
    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &Self::UsagePayload,
    ) -> Result<()> {
        ProgramInteractionPolicyExt::validate_payload(self, context, payload)
    }

    // Wrapper method to distinguish between transaction and sync transaction payloads
    fn execute_payload<'info>(
        &mut self,
        args: Self::ExecutionArgs,
        payload: &Self::UsagePayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        match &payload.transaction_payload {
            ProgramInteractionTransactionPayload::AsyncTransaction(..) => {
                execute_payload_async(self, args, payload, accounts)
            }
            ProgramInteractionTransactionPayload::SyncTransaction(..) => {
                execute_payload_sync(self, args, payload, accounts)
            }
        }
    }
}

// =============================================================================
// ASYNC EXECUTION
// =============================================================================

fn execute_payload_async<'info>(
    policy: &mut ProgramInteractionPolicy,
    args: ProgramInteractionExecutionArgs,
    payload: &ProgramInteractionPayload,
    mut accounts: &'info [AccountInfo<'info>],
) -> Result<()> {
    let transaction_payload = payload.get_transaction_payload(args.transaction_key)?;

    let smart_account_seeds = &[
        SEED_PREFIX,
        args.settings_key.as_ref(),
        SEED_SMART_ACCOUNT,
        &transaction_payload.account_index.to_le_bytes(),
    ];
    let (smart_account_pubkey, smart_account_bump) =
        Pubkey::find_program_address(smart_account_seeds, &crate::ID);

    let smart_account_signer_seeds = &[
        smart_account_seeds[0],
        smart_account_seeds[1],
        smart_account_seeds[2],
        smart_account_seeds[3],
        &[smart_account_bump],
    ];

    let (pre_hook_accounts, post_hook_accounts) = policy.parse_hook_accounts(&mut accounts);

    let num_lookups = transaction_payload.message.address_table_lookups.len();
    let message_account_infos = accounts
        .get(num_lookups..)
        .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;
    let address_lookup_table_account_infos = accounts
        .get(..num_lookups)
        .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;

    if let Some(instruction_constraint_indices) = &payload.instruction_constraint_indices {
        policy.evaluate_instruction_constraints(
            instruction_constraint_indices,
            &transaction_payload.message.instructions,
            message_account_infos,
        )?;
    }

    if let Some(pre_hook) = &policy.pre_hook {
        pre_hook.execute(
            pre_hook_accounts,
            &transaction_payload.message.instructions,
            &accounts[num_lookups..],
        )?;
    }
    let (ephemeral_signer_keys, ephemeral_signer_seeds) = derive_ephemeral_signers(
        args.transaction_key,
        &transaction_payload.ephemeral_signer_bumps,
    );

    let executable_message = ExecutableTransactionMessage::new_validated(
        transaction_payload.message.clone(),
        message_account_infos,
        address_lookup_table_account_infos,
        &smart_account_pubkey,
        &ephemeral_signer_keys,
    )?;

    let protected_accounts = &[args.proposal_key];

    if !policy.spending_limits.is_empty() {
        let current_timestamp = Clock::get()?.unix_timestamp;
        for spending_limit in &mut policy.spending_limits {
            spending_limit.reset_if_needed(current_timestamp);
        }

        let tracked_pre_balances = check_pre_balances(smart_account_pubkey, accounts);
        executable_message.execute_message(
            smart_account_signer_seeds,
            &ephemeral_signer_seeds,
            protected_accounts,
        )?;
        tracked_pre_balances.evaluate_balance_changes(&mut policy.spending_limits)?;
    } else {
        executable_message.execute_message(
            smart_account_signer_seeds,
            &ephemeral_signer_seeds,
            protected_accounts,
        )?;
    }

    if let Some(post_hook) = &policy.post_hook {
        post_hook.execute(
            post_hook_accounts,
            &transaction_payload.message.instructions,
            &accounts[num_lookups..],
        )?;
    }
    Ok(())
}

// =============================================================================
// SYNC EXECUTION
// =============================================================================

fn execute_payload_sync<'info>(
    policy: &mut ProgramInteractionPolicy,
    args: ProgramInteractionExecutionArgs,
    payload: &ProgramInteractionPayload,
    mut accounts: &'info [AccountInfo<'info>],
) -> Result<()> {
    let sync_transaction_payload = payload.get_sync_transaction_payload()?;
    let settings_key = args.settings_key;
    let instructions =
        SmallVec::<u8, CompiledInstruction>::try_from_slice(&sync_transaction_payload.instructions)
            .map_err(|_| SmartAccountError::InvalidInstructionArgs)?;

    let settings_compiled_instructions: Vec<SmartAccountCompiledInstruction> =
        Vec::from(instructions)
            .into_iter()
            .map(SmartAccountCompiledInstruction::from)
            .collect();
    let smart_account_seeds = &[
        SEED_PREFIX,
        settings_key.as_ref(),
        SEED_SMART_ACCOUNT,
        &sync_transaction_payload.account_index.to_le_bytes(),
    ];
    let (smart_account_pubkey, smart_account_bump) =
        Pubkey::find_program_address(smart_account_seeds, &crate::ID);

    let smart_account_signer_seeds = &[
        smart_account_seeds[0],
        smart_account_seeds[1],
        smart_account_seeds[2],
        smart_account_seeds[3],
        &[smart_account_bump],
    ];

    let (pre_hook_accounts, post_hook_accounts) = policy.parse_hook_accounts(&mut accounts);

    if let Some(instruction_constraint_indices) = &payload.instruction_constraint_indices {
        policy.evaluate_instruction_constraints(
            instruction_constraint_indices,
            &settings_compiled_instructions,
            accounts,
        )?;
    }

    if let Some(pre_hook) = &policy.pre_hook {
        pre_hook.execute(pre_hook_accounts, &settings_compiled_instructions, accounts)?;
    }

    let executable_message = SynchronousTransactionMessage::new_validated(
        &settings_key,
        &smart_account_pubkey,
        &args.policy_signers,
        &settings_compiled_instructions,
        accounts,
    )?;

    if !policy.spending_limits.is_empty() {
        let current_timestamp = Clock::get()?.unix_timestamp;
        for spending_limit in &mut policy.spending_limits {
            spending_limit.reset_if_needed(current_timestamp);
        }

        let tracked_pre_balances = check_pre_balances(smart_account_pubkey, accounts);
        executable_message.execute(smart_account_signer_seeds)?;
        tracked_pre_balances.evaluate_balance_changes(&mut policy.spending_limits)?;
    } else {
        executable_message.execute(smart_account_signer_seeds)?;
    }

    if let Some(post_hook) = &policy.post_hook {
        post_hook.execute(
            post_hook_accounts,
            &settings_compiled_instructions,
            accounts,
        )?;
    }

    Ok(())
}
