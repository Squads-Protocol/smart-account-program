use crate::{
    errors::*,
    state::policies::utils::{
        check_pre_balances, PeriodV2, QuantityConstraints, SpendingLimitV2, TimeConstraints,
        UsageState,
    },
    utils::{
        derive_ephemeral_signers, ExecutableTransactionMessage, SynchronousTransactionMessage,
    },
    CompiledInstruction, PolicyExecutionContext, PolicyPayloadConversionTrait, PolicySizeTrait,
    PolicyTrait, SmallVec, SmartAccountCompiledInstruction, SmartAccountSigner, TransactionMessage,
    TransactionPayload, TransactionPayloadDetails, HOOK_AUTHORITY_PUBKEY, SEED_EPHEMERAL_SIGNER,
    SEED_HOOK_AUTHORITY, SEED_PREFIX, SEED_SMART_ACCOUNT,
};
use anchor_lang::prelude::*;
use solana_program::instruction::Instruction;

// =============================================================================
// CORE POLICY STRUCTURES
// =============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ProgramInteractionPolicy {
    /// The account index of the account that will be used to execute the policy
    pub account_index: u8,
    /// Deduplicated pubkey table
    pub pubkey_table: Vec<Pubkey>,
    /// Constraints evaluated as a logical OR
    pub instructions_constraints: Vec<InstructionConstraint>,
    /// Hook invoked before inner instruction execution
    pub pre_hook: Option<Hook>,
    /// Hook invoked after inner instruction execution
    pub post_hook: Option<Hook>,
    /// Spending limits applied during policy execution
    pub spending_limits: Vec<SpendingLimitV2>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct InstructionConstraint {
    /// Index into pubkey_table for the program_id
    pub program_id_index: u8,
    /// Account constraints (evaluated as logical AND)
    pub account_constraints: Vec<AccountConstraint>,
    /// Data constraints (evaluated as logical AND)
    pub data_constraints: Vec<DataConstraint>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct Hook {
    // Dictates how many extra accounts are required for the hook, beyond the program ID
    pub num_extra_accounts: u8,
    // Dictates constraints for the hook accounts
    pub account_constraints: Vec<AccountConstraint>,
    // Dictates which instruction data will be invoked
    pub instruction_data: Vec<u8>,
    // Index into pubkey_table for the program_id
    pub program_id_index: u8,
    // Dictates if inner instruction data & account will be passed to the
    // instruction on top of the instruction data
    pub pass_inner_instructions: bool,
}

impl Hook {
    pub fn size(&self) -> usize {
        1 + // num_accounts
        4 + self.account_constraints.iter().map(|c| c.size()).sum::<usize>() + // account_constraints vec
        4 + self.instruction_data.len() + // instruction_data
        1 + // program_id_index
        1 // pass_inner_instructions
    }

    // Get the total number of accounts required for the hook
    pub fn num_accounts(&self) -> usize {
        // Program ID also needs to get passed in, therefore add 1
        self.num_extra_accounts
            .checked_add(1)
            .map(|v| v as usize)
            .unwrap()
    }
}
// =============================================================================
// CONSTRAINT TYPES AND OPERATORS
// =============================================================================

impl InstructionConstraint {
    pub fn size(&self) -> usize {
        1 + // program_id_index
        4 + self.account_constraints.iter().map(|c| c.size()).sum::<usize>() + // account_constraints vec
        4 + self.data_constraints.iter().map(|c| c.size()).sum::<usize>() // data_constraints vec
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum DataOperator {
    Equals,
    NotEquals,
    GreaterThan,
    GreaterThanOrEqualTo,
    LessThan,
    LessThanOrEqualTo,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum DataValue {
    U8(u8),
    /// Little-endian u16
    U16Le(u16),
    /// Little-endian u32
    U32Le(u32),
    /// Little-endian u64
    U64Le(u64),
    /// Little-endian u128
    U128Le(u128),
    /// Byte slice for discriminators etc. Only supports Equals/NotEquals
    U8Slice(Vec<u8>),
}

impl DataValue {
    pub fn size(&self) -> usize {
        1 + // enum discriminator
        match self {
            DataValue::U8(_) => 1,
            DataValue::U16Le(_) => 2,
            DataValue::U32Le(_) => 4,
            DataValue::U64Le(_) => 8,
            DataValue::U128Le(_) => 16,
            DataValue::U8Slice(bytes) => 4 + bytes.len(), // vec length + bytes
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct DataConstraint {
    pub data_offset: u64,
    pub data_value: DataValue,
    pub operator: DataOperator,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum AccountConstraintType {
    Pubkey(Vec<u8>),
    AccountData(Vec<DataConstraint>),
}

impl AccountConstraintType {
    pub fn size(&self) -> usize {
        match self {
            AccountConstraintType::Pubkey(indices) => 1 + 4 + indices.len(),
            AccountConstraintType::AccountData(constraints) => {
                1 + 4 + constraints.iter().map(|c| c.size()).sum::<usize>()
            }
        }
    }
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct AccountConstraint {
    pub account_index: u8,
    pub account_constraint: AccountConstraintType,
    pub owner_index: Option<u8>,
}

// =============================================================================
// SIZE CALCULATIONS
// =============================================================================

impl DataConstraint {
    pub fn size(&self) -> usize {
        8 + // data_offset
        self.data_value.size() + // data_value
        1 // operator
    }
}

impl AccountConstraint {
    pub fn size(&self) -> usize {
        1 + // account_index
        4 + self.account_constraint.size() + // account_constraint
        1 + // Option<u8> discriminator
        if self.owner_index.is_some() { 1 } else { 0 } // owner_index value
    }
}

// =============================================================================
// CONSTRAINT EVALUATION LOGIC
// =============================================================================

impl DataConstraint {
    /// Evaluate constraint against instruction data
    pub fn evaluate(&self, data: &[u8]) -> Result<()> {
        let offset = self.data_offset as usize;

        let constraint_passed = match &self.data_value {
            DataValue::U8(expected) => {
                // Check bounds
                if offset >= data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort.into());
                }
                let actual = data[offset];
                self.compare(actual, *expected)?
            }
            DataValue::U16Le(expected) => {
                // Check bounds for 2 bytes
                if offset + 2 > data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort.into());
                }
                let bytes = &data[offset..offset + 2];
                let actual = u16::from_le_bytes([bytes[0], bytes[1]]);
                self.compare(actual, *expected)?
            }
            DataValue::U32Le(expected) => {
                // Check bounds for 4 bytes
                if offset + 4 > data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort.into());
                }
                let bytes = &data[offset..offset + 4];
                let actual = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                self.compare(actual, *expected)?
            }
            DataValue::U64Le(expected) => {
                // Check bounds for 8 bytes
                if offset + 8 > data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort.into());
                }
                let actual = u64::from_le_bytes(
                    data[offset..offset + 8]
                        .try_into()
                        .map_err(|_| SmartAccountError::ProgramInteractionDataParsingError)?,
                );
                self.compare(actual, *expected)?
            }
            DataValue::U128Le(expected) => {
                // Check bounds for 16 bytes
                if offset + 16 > data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort.into());
                }
                let actual = u128::from_le_bytes(
                    data[offset..offset + 16]
                        .try_into()
                        .map_err(|_| SmartAccountError::ProgramInteractionDataParsingError)?,
                );
                self.compare(actual, *expected)?
            }
            DataValue::U8Slice(expected) => {
                // Check bounds for slice length
                if offset + expected.len() > data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort.into());
                }
                let actual = &data[offset..offset + expected.len()];
                match self.operator {
                    DataOperator::Equals => actual == expected.as_slice(),
                    DataOperator::NotEquals => actual != expected.as_slice(),
                    _ => {
                        return Err(
                            SmartAccountError::ProgramInteractionUnsupportedSliceOperator.into(),
                        );
                    }
                }
            }
        };

        if constraint_passed {
            Ok(())
        } else {
            Err(SmartAccountError::ProgramInteractionInvalidNumericValue.into())
        }
    }

    /// Compare two values using the specified operator
    fn compare<T: PartialOrd + PartialEq>(&self, actual: T, expected: T) -> Result<bool> {
        Ok(match self.operator {
            DataOperator::Equals => actual == expected,
            DataOperator::NotEquals => actual != expected,
            DataOperator::GreaterThan => actual > expected,
            DataOperator::GreaterThanOrEqualTo => actual >= expected,
            DataOperator::LessThan => actual < expected,
            DataOperator::LessThanOrEqualTo => actual <= expected,
        })
    }
}


// =============================================================================
// CREATION PAYLOAD TYPES
// =============================================================================

/// Limited subset of TimeConstraints
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct LimitedTimeConstraints {
    pub start: i64,
    pub expiration: Option<i64>,
    pub period: PeriodV2,
}

/// Limited subset of QuantityConstraints
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct LimitedQuantityConstraints {
    pub max_per_period: u64,
}

/// Limited subset of BalanceConstraint used to create a policy
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct LimitedSpendingLimit {
    pub mint: Pubkey,
    pub time_constraints: LimitedTimeConstraints,
    pub quantity_constraints: LimitedQuantityConstraints,
}

/// Creation payload version of InstructionConstraint (uses full Pubkeys)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct InstructionConstraintCreationPayload {
    pub program_id: Pubkey,
    pub account_constraints: Vec<AccountConstraintCreationPayload>,
    pub data_constraints: Vec<DataConstraint>,
}

/// Creation payload version of Hook (uses full Pubkeys)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct HookCreationPayload {
    pub num_extra_accounts: u8,
    pub account_constraints: Vec<AccountConstraintCreationPayload>,
    pub instruction_data: Vec<u8>,
    pub program_id: Pubkey,
    pub pass_inner_instructions: bool,
}

/// Creation payload version of AccountConstraint (uses full Pubkeys)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct AccountConstraintCreationPayload {
    pub account_index: u8,
    pub account_constraint: AccountConstraintTypeCreationPayload,
    pub owner: Option<Pubkey>,
}

/// Creation payload version of AccountConstraintType (uses full Pubkeys)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum AccountConstraintTypeCreationPayload {
    Pubkey(Vec<Pubkey>),
    AccountData(Vec<DataConstraint>),
}

/// Payload used to create a program interaction policy
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ProgramInteractionPolicyCreationPayload {
    pub account_index: u8,
    pub instructions_constraints: Vec<InstructionConstraintCreationPayload>,
    pub pre_hook: Option<HookCreationPayload>,
    pub post_hook: Option<HookCreationPayload>,
    pub spending_limits: Vec<LimitedSpendingLimit>,
}

// =============================================================================
// TRANSACTION PAYLOAD TYPES
// =============================================================================
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ProgramInteractionPayload {
    pub instruction_constraint_indices: Option<Vec<u8>>,
    pub transaction_payload: ProgramInteractionTransactionPayload,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum ProgramInteractionTransactionPayload {
    AsyncTransaction(TransactionPayload),
    SyncTransaction(SyncTransactionPayloadDetails),
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SyncTransactionPayloadDetails {
    pub account_index: u8,
    pub instructions: Vec<u8>,
}

pub struct ProgramInteractionExecutionArgs {
    pub settings_key: Pubkey,
    pub transaction_key: Pubkey,
    pub proposal_key: Pubkey,
    pub policy_signers: Vec<SmartAccountSigner>,
}

// =============================================================================
// CORE POLICY IMPLEMENTATION
// =============================================================================

impl Hook {
    pub fn execute<'info>(
        &self,
        policy: &ProgramInteractionPolicy,
        hook_accounts: &'info [AccountInfo<'info>],
        instructions: &[SmartAccountCompiledInstruction],
        instruction_accounts: &[AccountInfo<'info>],
    ) -> Result<()> {
        use borsh::BorshSerialize;

        // Evaluate the hook accounts
        for account_constraint in self.account_constraints.iter() {
            let account = &hook_accounts[account_constraint.account_index as usize];
            policy.evaluate_account_constraint(account_constraint, account)?;
        }

        // Resolve the hook program_id
        let program_id = policy.resolve_pubkey(self.program_id_index)?;

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
            program_id: *program_id,
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
impl ProgramInteractionPolicy {
    /// Evaluate the instruction constraints for a given instruction
    pub fn evaluate_instruction_constraints<'info>(
        &self,
        instruction_constraint_indices: &[u8],
        instructions: &[SmartAccountCompiledInstruction],
        accounts: &[AccountInfo<'info>],
    ) -> Result<()> {
        // Iterate over instruction and their corresponding instruction constraint
        for (instruction, instruction_constraint_index) in
            instructions.iter().zip(instruction_constraint_indices)
        {
            let instruction_constraint =
                &self.instructions_constraints[*instruction_constraint_index as usize];

            // Evaluate the program id constraint
            let program_id = self.resolve_pubkey(instruction_constraint.program_id_index)?;
            require!(
                accounts[instruction.program_id_index as usize].key == program_id,
                SmartAccountError::ProgramInteractionProgramIdMismatch
            );

            // Evaluate the account constraints
            for account_constraint in &instruction_constraint.account_constraints {
                let mapped_account_index = instruction.account_indexes[account_constraint.account_index as usize];
                let account = &accounts[mapped_account_index as usize];
                self.evaluate_account_constraint(account_constraint, account)?;
            }

            // Evaluate the data constraints
            for data_constraint in &instruction_constraint.data_constraints {
                data_constraint.evaluate(instruction.data.as_slice())?;
            }
        }
        Ok(())
    }

    // Parses hook accounts from the accounts slice and returns them
    pub fn parse_hook_accounts<'info, 'a>(
        &self,
        accounts: &mut &'a [AccountInfo<'info>],
    ) -> (&'a [AccountInfo<'info>], &'a [AccountInfo<'info>]) {
        // Split all accounts into pre hook accounts, post_hook accounts and
        // transaction related accounts including lookups
        let mut pre_hook_accounts_intermediate: &[AccountInfo<'info>] = &[];
        let mut post_hook_accounts_intermediate: &[AccountInfo<'info>] = &[];
        let mut transaction_accounts = *accounts;

        if self.pre_hook.is_some() {
            let (pre_hook_accounts, remaining_accounts) = transaction_accounts
                .split_at(self.pre_hook.as_ref().unwrap().num_accounts() as usize);
            pre_hook_accounts_intermediate = pre_hook_accounts;
            transaction_accounts = remaining_accounts;
        };
        if self.post_hook.is_some() {
            let (post_hook_accounts, remaining_accounts) = transaction_accounts
                .split_at(self.post_hook.as_ref().unwrap().num_accounts() as usize);
            post_hook_accounts_intermediate = post_hook_accounts;
            transaction_accounts = remaining_accounts;
        }

        // Re-set transaction accounts
        *accounts = transaction_accounts;

        (
            pre_hook_accounts_intermediate,
            post_hook_accounts_intermediate,
        )
    }

    #[inline]
    fn resolve_pubkey(&self, index: u8) -> Result<&Pubkey> {
        self.pubkey_table
            .get(index as usize)
            .ok_or_else(|| SmartAccountError::ProgramInteractionInvalidPubkeyTableIndex.into())
    }

    fn evaluate_account_constraint(
        &self,
        constraint: &AccountConstraint,
        account: &AccountInfo,
    ) -> Result<()> {
        // Evaluate owner constraint
        if let Some(owner_index) = constraint.owner_index {
            let owner = self.resolve_pubkey(owner_index)?;
            require_eq!(
                account.owner,
                owner,
                SmartAccountError::IllegalAccountOwner
            );
        }

        // Evaluate account constraint type
        match &constraint.account_constraint {
            AccountConstraintType::Pubkey(indices) => {
                let found = indices.iter().any(|&index| {
                    self.resolve_pubkey(index)
                        .map(|pk| account.key == pk)
                        .unwrap_or(false)
                });

                require!(
                    found,
                    SmartAccountError::ProgramInteractionAccountConstraintViolated
                );
            }
            AccountConstraintType::AccountData(constraints) => {
                let data = account.try_borrow_data()?;
                for data_constraint in constraints {
                    data_constraint.evaluate(&data)?;
                }
            }
        }
        Ok(())
    }
}

// =============================================================================
// SIZE IMPLEMENTATIONS FOR CREATION PAYLOAD TYPES
// =============================================================================

impl LimitedTimeConstraints {
    pub fn size(&self) -> usize {
        8 + // start
        1 + // option discriminator for expiration
        match self.expiration {
            Some(_) => 8, // expiration value
            None => 0,
        } +
        1 // period enum discriminator (PeriodV2 is small enum)
    }
}

impl LimitedQuantityConstraints {
    pub fn size(&self) -> usize {
        8 // max_per_period
    }
}

impl LimitedSpendingLimit {
    pub fn size(&self) -> usize {
        32 + // mint
        self.time_constraints.size() + // time_constraints
        self.quantity_constraints.size() // quantity_constraints
    }
}

impl InstructionConstraintCreationPayload {
    pub fn size(&self) -> usize {
        32 + // program_id
        4 + self.account_constraints.iter().map(|c| c.size()).sum::<usize>() + // account_constraints vec
        4 + self.data_constraints.iter().map(|c| c.size()).sum::<usize>() // data_constraints vec
    }
}

impl HookCreationPayload {
    pub fn size(&self) -> usize {
        1 + // num_extra_accounts
        4 + self.account_constraints.iter().map(|c| c.size()).sum::<usize>() + // account_constraints vec
        4 + self.instruction_data.len() + // instruction_data
        32 + // program_id
        1 // pass_inner_instructions
    }
}

impl AccountConstraintCreationPayload {
    pub fn size(&self) -> usize {
        1 + // account_index
        4 + self.account_constraint.size() + // account_constraint
        1 + // owner Option discriminator
        match self.owner {
            Some(_) => 32, // owner pubkey
            None => 0,
        }
    }
}

impl AccountConstraintTypeCreationPayload {
    pub fn size(&self) -> usize {
        match self {
            AccountConstraintTypeCreationPayload::Pubkey(pubkeys) => 1 + 4 + (pubkeys.len() * 32),
            AccountConstraintTypeCreationPayload::AccountData(constraints) => {
                1 + 4 + constraints.iter().map(|c| c.size()).sum::<usize>()
            }
        }
    }
}

// =============================================================================
// PAYLOAD CONVERSION IMPLEMENTATIONS
// =============================================================================

impl PolicyPayloadConversionTrait for ProgramInteractionPolicyCreationPayload {
    type PolicyState = ProgramInteractionPolicy;

    fn to_policy_state(self) -> Result<ProgramInteractionPolicy> {
        // For sanity sake, we limit the number of instruction constraints and
        // spending limits
        require!(
            self.instructions_constraints.len() <= 20,
            SmartAccountError::ProgramInteractionTooManyInstructionConstraints
        );
        require!(
            self.spending_limits.len() <= 10,
            SmartAccountError::ProgramInteractionTooManySpendingLimits
        );

        // Build deduplicated pubkey table
        use std::collections::HashMap;
        let mut pubkey_table = Vec::new();
        let mut pubkey_to_index: HashMap<Pubkey, u8> = HashMap::new();

        // Helper closure to get or insert a pubkey into the table
        let mut get_or_insert_index = |pubkey: &Pubkey| -> Result<u8> {
            if let Some(&index) = pubkey_to_index.get(pubkey) {
                return Ok(index);
            }
            let index = pubkey_table.len() as u8;
            require!(
                pubkey_table.len() < 256,
                SmartAccountError::ProgramInteractionTooManyUniquePubkeys
            );
            pubkey_table.push(*pubkey);
            pubkey_to_index.insert(*pubkey, index);
            Ok(index)
        };

        // Convert instruction constraints
        let mut instructions_constraints = Vec::new();
        for constraint in self.instructions_constraints {
            let program_id_index = get_or_insert_index(&constraint.program_id)?;

            let mut account_constraints = Vec::new();
            for ac in constraint.account_constraints {
                let owner_index = if let Some(owner) = ac.owner {
                    Some(get_or_insert_index(&owner)?)
                } else {
                    None
                };

                let account_constraint = match ac.account_constraint {
                    AccountConstraintTypeCreationPayload::Pubkey(pubkeys) => {
                        let mut indices = Vec::new();
                        for pk in pubkeys {
                            indices.push(get_or_insert_index(&pk)?);
                        }
                        AccountConstraintType::Pubkey(indices)
                    }
                    AccountConstraintTypeCreationPayload::AccountData(data_constraints) => {
                        AccountConstraintType::AccountData(data_constraints)
                    }
                };

                account_constraints.push(AccountConstraint {
                    account_index: ac.account_index,
                    account_constraint,
                    owner_index,
                });
            }

            instructions_constraints.push(InstructionConstraint {
                program_id_index,
                account_constraints,
                data_constraints: constraint.data_constraints,
            });
        }

        // Convert pre_hook
        let pre_hook = if let Some(hook) = self.pre_hook {
            let program_id_index = get_or_insert_index(&hook.program_id)?;

            let mut account_constraints = Vec::new();
            for ac in hook.account_constraints {
                let owner_index = if let Some(owner) = ac.owner {
                    Some(get_or_insert_index(&owner)?)
                } else {
                    None
                };

                let account_constraint = match ac.account_constraint {
                    AccountConstraintTypeCreationPayload::Pubkey(pubkeys) => {
                        let mut indices = Vec::new();
                        for pk in pubkeys {
                            indices.push(get_or_insert_index(&pk)?);
                        }
                        AccountConstraintType::Pubkey(indices)
                    }
                    AccountConstraintTypeCreationPayload::AccountData(data_constraints) => {
                        AccountConstraintType::AccountData(data_constraints)
                    }
                };

                account_constraints.push(AccountConstraint {
                    account_index: ac.account_index,
                    account_constraint,
                    owner_index,
                });
            }

            Some(Hook {
                num_extra_accounts: hook.num_extra_accounts,
                account_constraints,
                instruction_data: hook.instruction_data,
                program_id_index,
                pass_inner_instructions: hook.pass_inner_instructions,
            })
        } else {
            None
        };

        // Convert post_hook
        let post_hook = if let Some(hook) = self.post_hook {
            let program_id_index = get_or_insert_index(&hook.program_id)?;

            let mut account_constraints = Vec::new();
            for ac in hook.account_constraints {
                let owner_index = if let Some(owner) = ac.owner {
                    Some(get_or_insert_index(&owner)?)
                } else {
                    None
                };

                let account_constraint = match ac.account_constraint {
                    AccountConstraintTypeCreationPayload::Pubkey(pubkeys) => {
                        let mut indices = Vec::new();
                        for pk in pubkeys {
                            indices.push(get_or_insert_index(&pk)?);
                        }
                        AccountConstraintType::Pubkey(indices)
                    }
                    AccountConstraintTypeCreationPayload::AccountData(data_constraints) => {
                        AccountConstraintType::AccountData(data_constraints)
                    }
                };

                account_constraints.push(AccountConstraint {
                    account_index: ac.account_index,
                    account_constraint,
                    owner_index,
                });
            }

            Some(Hook {
                num_extra_accounts: hook.num_extra_accounts,
                account_constraints,
                instruction_data: hook.instruction_data,
                program_id_index,
                pass_inner_instructions: hook.pass_inner_instructions,
            })
        } else {
            None
        };

        // Convert spending limits
        let mut spending_limits = self.spending_limits.clone();
        spending_limits.sort_by_key(|c| c.mint);

        let current_timestamp = Clock::get()?.unix_timestamp;

        Ok(ProgramInteractionPolicy {
            account_index: self.account_index,
            pubkey_table,
            instructions_constraints,
            pre_hook,
            post_hook,
            spending_limits: spending_limits
                .iter()
                .map(|spending_limit| {
                    // Determine the start timestamp
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
}

impl PolicySizeTrait for ProgramInteractionPolicyCreationPayload {
    fn creation_payload_size(&self) -> usize {
        1 + // account_scope
        4 + self.instructions_constraints.iter().map(|c| c.size()).sum::<usize>() + // instructions_constraints vec
        1 + self.pre_hook.as_ref().map(|h| h.size()).unwrap_or(0) + // pre_hook
        1 + self.post_hook.as_ref().map(|h| h.size()).unwrap_or(0) + // post_hook
        4 + self.spending_limits.iter().map(|constraint| constraint.size()).sum::<usize>()
        // spending_limits vec
    }

    fn policy_state_size(&self) -> usize {
        use std::collections::HashSet;

        // Collect all unique pubkeys to calculate pubkey_table size
        let mut unique_pubkeys = HashSet::new();

        // Collect from instruction constraints
        for constraint in &self.instructions_constraints {
            unique_pubkeys.insert(constraint.program_id);
            for ac in &constraint.account_constraints {
                if let Some(owner) = ac.owner {
                    unique_pubkeys.insert(owner);
                }
                if let AccountConstraintTypeCreationPayload::Pubkey(pubkeys) = &ac.account_constraint {
                    for pk in pubkeys {
                        unique_pubkeys.insert(*pk);
                    }
                }
            }
        }

        // Collect from pre_hook
        if let Some(hook) = &self.pre_hook {
            unique_pubkeys.insert(hook.program_id);
            for ac in &hook.account_constraints {
                if let Some(owner) = ac.owner {
                    unique_pubkeys.insert(owner);
                }
                if let AccountConstraintTypeCreationPayload::Pubkey(pubkeys) = &ac.account_constraint {
                    for pk in pubkeys {
                        unique_pubkeys.insert(*pk);
                    }
                }
            }
        }

        // Collect from post_hook
        if let Some(hook) = &self.post_hook {
            unique_pubkeys.insert(hook.program_id);
            for ac in &hook.account_constraints {
                if let Some(owner) = ac.owner {
                    unique_pubkeys.insert(owner);
                }
                if let AccountConstraintTypeCreationPayload::Pubkey(pubkeys) = &ac.account_constraint {
                    for pk in pubkeys {
                        unique_pubkeys.insert(*pk);
                    }
                }
            }
        }

        let pubkey_table_size = 4 + (unique_pubkeys.len() * 32); // vec length + pubkeys

        // Calculate size of converted instruction constraints (using indices instead of pubkeys)
        let instructions_constraints_size: usize = self.instructions_constraints.iter().map(|c| {
            1 + // program_id_index
            4 + c.account_constraints.iter().map(|ac| {
                1 + // account_index
                4 + match &ac.account_constraint {
                    AccountConstraintTypeCreationPayload::Pubkey(pubkeys) => 1 + 4 + pubkeys.len(), // enum discriminator + vec length + indices
                    AccountConstraintTypeCreationPayload::AccountData(constraints) => {
                        1 + 4 + constraints.iter().map(|dc| dc.size()).sum::<usize>()
                    }
                } +
                1 + // owner_index Option discriminator
                match ac.owner {
                    Some(_) => 1, // owner_index
                    None => 0,
                }
            }).sum::<usize>() + // account_constraints vec
            4 + c.data_constraints.iter().map(|dc| dc.size()).sum::<usize>() // data_constraints vec
        }).sum();

        // Calculate size of converted hooks
        let pre_hook_size = self.pre_hook.as_ref().map(|h| {
            1 + // num_extra_accounts
            4 + h.account_constraints.iter().map(|ac| {
                1 + // account_index
                4 + match &ac.account_constraint {
                    AccountConstraintTypeCreationPayload::Pubkey(pubkeys) => 1 + 4 + pubkeys.len(),
                    AccountConstraintTypeCreationPayload::AccountData(constraints) => {
                        1 + 4 + constraints.iter().map(|dc| dc.size()).sum::<usize>()
                    }
                } +
                1 + // owner_index Option discriminator
                match ac.owner {
                    Some(_) => 1,
                    None => 0,
                }
            }).sum::<usize>() + // account_constraints vec
            4 + h.instruction_data.len() + // instruction_data
            1 + // program_id_index
            1 // pass_inner_instructions
        }).unwrap_or(0);

        let post_hook_size = self.post_hook.as_ref().map(|h| {
            1 + // num_extra_accounts
            4 + h.account_constraints.iter().map(|ac| {
                1 + // account_index
                4 + match &ac.account_constraint {
                    AccountConstraintTypeCreationPayload::Pubkey(pubkeys) => 1 + 4 + pubkeys.len(),
                    AccountConstraintTypeCreationPayload::AccountData(constraints) => {
                        1 + 4 + constraints.iter().map(|dc| dc.size()).sum::<usize>()
                    }
                } +
                1 + // owner_index Option discriminator
                match ac.owner {
                    Some(_) => 1,
                    None => 0,
                }
            }).sum::<usize>() + // account_constraints vec
            4 + h.instruction_data.len() + // instruction_data
            1 + // program_id_index
            1 // pass_inner_instructions
        }).unwrap_or(0);

        1 + // account_index
        pubkey_table_size + // pubkey_table vec
        4 + instructions_constraints_size + // instructions_constraints vec
        1 + pre_hook_size + // pre_hook
        1 + post_hook_size + // post_hook
        4 + self.spending_limits.iter().map(|_| SpendingLimitV2::INIT_SPACE).sum::<usize>()
        // spending_limits vec
    }
}

// =============================================================================
// TRANSACTION PAYLOAD IMPLEMENTATIONS
// =============================================================================
impl ProgramInteractionPayload {
    /// Get the async transaction payload details for the policy
    pub fn get_transaction_payload(
        &self,
        transaction_key: Pubkey,
    ) -> Result<TransactionPayloadDetails> {
        match &self.transaction_payload {
            ProgramInteractionTransactionPayload::AsyncTransaction(transaction_payload) => {
                // Deserialize the transaction message
                let transaction_message = TransactionMessage::deserialize(
                    &mut transaction_payload.transaction_message.as_slice(),
                )?;
                // Derive the ephemeral signer bumps
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

                // Create the transaction payload details
                Ok(TransactionPayloadDetails {
                    account_index: transaction_payload.account_index,
                    ephemeral_signer_bumps,
                    message: transaction_message.try_into()?,
                })
            }
            _ => Err(SmartAccountError::InvalidPayload.into()),
        }
    }

    /// Get the sync transaction payload details for the policy
    pub fn get_sync_transaction_payload(&self) -> Result<&SyncTransactionPayloadDetails> {
        match &self.transaction_payload {
            ProgramInteractionTransactionPayload::SyncTransaction(sync_transaction_payload) => {
                Ok(sync_transaction_payload)
            }
            _ => Err(SmartAccountError::InvalidPayload.into()),
        }
    }
}

impl ProgramInteractionTransactionPayload {
    /// Get the account index for the transaction payload
    pub fn get_account_index(&self) -> u8 {
        match self {
            ProgramInteractionTransactionPayload::AsyncTransaction(transaction_payload) => {
                transaction_payload.account_index
            }
            ProgramInteractionTransactionPayload::SyncTransaction(sync_transaction_payload) => {
                sync_transaction_payload.account_index
            }
        }
    }

    /// Get the number of instructions for the transaction payload
    pub fn instructions_len(&self) -> Result<usize> {
        match self {
            ProgramInteractionTransactionPayload::AsyncTransaction(transaction_payload) => {
                // TODO: Inefficient to deserialize the transaction message and
                // not do anything with it.
                Ok(TransactionMessage::deserialize(
                    &mut transaction_payload.transaction_message.as_slice(),
                )
                .map_err(|_| SmartAccountError::InvalidInstructionArgs)?
                .instructions
                .len())
            }
            ProgramInteractionTransactionPayload::SyncTransaction(sync_transaction_payload) => {
                // Since its a small vec, we can get the length directly from
                // the first byte
                Ok(sync_transaction_payload.instructions[0] as usize)
            }
        }
    }
}

// =============================================================================
// POLICY TRAIT IMPLEMENTATION
// =============================================================================

impl PolicyTrait for ProgramInteractionPolicy {
    type PolicyState = Self;
    type CreationPayload = ProgramInteractionPolicyCreationPayload;
    type UsagePayload = ProgramInteractionPayload;
    type ExecutionArgs = ProgramInteractionExecutionArgs;

    /// Validate the policy invariant
    fn invariant(&self) -> Result<()> {
        // There can't be duplicate balance constraint for the same mint
        // Assumes that the balance constraints are sorted by mint
        let has_duplicate = self
            .spending_limits
            .windows(2)
            .any(|window| window[0].mint == window[1].mint);
        require!(
            !has_duplicate,
            SmartAccountError::ProgramInteractionDuplicateSpendingLimit
        );

        // Each spending limits invariant must be valid
        for spending_limit in &self.spending_limits {
            spending_limit.invariant()?;
        }

        Ok(())
    }

    /// Validate the payload for the policy
    fn validate_payload(
        &self,
        context: PolicyExecutionContext,
        payload: &Self::UsagePayload,
    ) -> Result<()> {
        // Validate that the payload is valid for the context
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
            // Both other variants are valid
            (_, _) => {}
        }

        // Get the account index and instructions length
        let payload_account_index = payload.transaction_payload.get_account_index();
        let instructions_len = match &payload.transaction_payload {
            ProgramInteractionTransactionPayload::AsyncTransaction(transaction_payload) => {
                // TODO: Inefficient to deserialize the transaction message and
                // not do anything with it.
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

        // If there are instruction constraints, ensure that the submitted instruction constraints are valid
        if !self.instructions_constraints.is_empty() {
            if let Some(instruction_constraint_indices) = &payload.instruction_constraint_indices {
                // Ensure that the instruction indices match the number of
                // instructions
                require_eq!(
                    instruction_constraint_indices.len(),
                    instructions_len,
                    SmartAccountError::ProgramInteractionInstructionCountMismatch
                );
                // Ensure that the instruction constraint index is within the bounds
                // of the instructions constraints
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

    // Wrapper method to distinguish between transaction and sync transaction payloads
    fn execute_payload<'info>(
        &mut self,
        args: Self::ExecutionArgs,
        payload: &Self::UsagePayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        match &payload.transaction_payload {
            ProgramInteractionTransactionPayload::AsyncTransaction(..) => {
                self.execute_payload_async(args, payload, accounts)
            }
            ProgramInteractionTransactionPayload::SyncTransaction(..) => {
                self.execute_payload_sync(args, payload, accounts)
            }
        }
    }
}

// =============================================================================
// ASYNC TRANSACTION EXECUTION
// =============================================================================

impl ProgramInteractionPolicy {
    /// Execute an async transaction through the policy
    fn execute_payload_async<'info>(
        &mut self,
        args: ProgramInteractionExecutionArgs,
        payload: &ProgramInteractionPayload,
        mut accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        // Get the transaction payload
        let transaction_payload = payload.get_transaction_payload(args.transaction_key)?;

        // Largely copied from `transaction_execute.rs`
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

        // Parse out the hook accounts from the accounts slice
        let (pre_hook_accounts, post_hook_accounts) = self.parse_hook_accounts(&mut accounts);

        // Get the message account infos and address lookup table account infos
        let num_lookups = transaction_payload.message.address_table_lookups.len();
        // Execute the transaction
        let message_account_infos = accounts
            .get(num_lookups..)
            .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;
        let address_lookup_table_account_infos = accounts
            .get(..num_lookups)
            .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;

        // Evaluate the instruction constraints
        if let Some(instruction_constraint_indices) = &payload.instruction_constraint_indices {
            self.evaluate_instruction_constraints(
                instruction_constraint_indices,
                &transaction_payload.message.instructions,
                message_account_infos,
            )?;
        }

        // Execute the pre hook
        if let Some(pre_hook) = &self.pre_hook {
            pre_hook.execute(
                self,
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

        // Update the spending limits if present
        if !self.spending_limits.is_empty() {
            let current_timestamp = Clock::get()?.unix_timestamp;
            // Reset the spending limits if needed
            for spending_limit in &mut self.spending_limits {
                spending_limit.reset_if_needed(current_timestamp);
            }

            let tracked_pre_balances = check_pre_balances(smart_account_pubkey, accounts);
            // Execute the transaction message instructions one-by-one.
            // NOTE: `execute_message()` calls `self.to_instructions_and_accounts()`
            // which in turn calls `take()` on
            // `self.message.instructions`, therefore after this point no more
            // references or usages of `self.message` should be made to avoid
            // faulty behavior.
            executable_message.execute_message(
                smart_account_signer_seeds,
                &ephemeral_signer_seeds,
                protected_accounts,
            )?;
            // Evaluate the balance changes post-execution
            tracked_pre_balances.evaluate_balance_changes(&mut self.spending_limits)?;
        } else {
            // Execute the transaction message instructions one-by-one.
            // NOTE: `execute_message()` calls `self.to_instructions_and_accounts()`
            // which in turn calls `take()` on
            // `self.message.instructions`, therefore after this point no more
            // references or usages of `self.message` should be made to avoid
            // faulty behavior.
            executable_message.execute_message(
                smart_account_signer_seeds,
                &ephemeral_signer_seeds,
                protected_accounts,
            )?;
        }

        // Execute post hook
        if let Some(post_hook) = &self.post_hook {
            post_hook.execute(
                self,
                post_hook_accounts,
                &transaction_payload.message.instructions,
                &accounts[num_lookups..],
            )?;
        }
        Ok(())
    }

    // =============================================================================
    // SYNC TRANSACTION EXECUTION
    // =============================================================================

    /// Execute a synchronous transaction through the policy
    fn execute_payload_sync<'info>(
        &mut self,
        args: ProgramInteractionExecutionArgs,
        payload: &ProgramInteractionPayload,
        mut accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        // Get the sync transaction payload
        let sync_transaction_payload = payload.get_sync_transaction_payload()?;
        // Get the settings key
        let settings_key = args.settings_key;
        // Validate the instructions
        let instructions = SmallVec::<u8, CompiledInstruction>::try_from_slice(
            &sync_transaction_payload.instructions,
        )
        .map_err(|_| SmartAccountError::InvalidInstructionArgs)?;

        // Convert to SmartAccountCompiledInstruction
        let settings_compiled_instructions: Vec<SmartAccountCompiledInstruction> =
            Vec::from(instructions)
                .into_iter()
                .map(SmartAccountCompiledInstruction::from)
                .collect();
        // Get the smart account seeds
        let smart_account_seeds = &[
            SEED_PREFIX,
            settings_key.as_ref(),
            SEED_SMART_ACCOUNT,
            &sync_transaction_payload.account_index.to_le_bytes(),
        ];
        let (smart_account_pubkey, smart_account_bump) =
            Pubkey::find_program_address(smart_account_seeds, &crate::ID);

        // Get the signer seeds for the smart account
        let smart_account_signer_seeds = &[
            smart_account_seeds[0],
            smart_account_seeds[1],
            smart_account_seeds[2],
            smart_account_seeds[3],
            &[smart_account_bump],
        ];

        // Parse out the hook accounts from the accounts slice
        let (pre_hook_accounts, post_hook_accounts) = self.parse_hook_accounts(&mut accounts);

        // Evaluate the instruction constraints
        if let Some(instruction_constraint_indices) = &payload.instruction_constraint_indices {
            self.evaluate_instruction_constraints(
                instruction_constraint_indices,
                &settings_compiled_instructions,
                accounts,
            )?;
        }

        // Execute the pre hook
        if let Some(pre_hook) = &self.pre_hook {
            pre_hook.execute(
                self,
                pre_hook_accounts,
                &settings_compiled_instructions,
                &accounts,
            )?;
        }

        let executable_message = SynchronousTransactionMessage::new_validated(
            &settings_key,
            &smart_account_pubkey,
            &args.policy_signers,
            &settings_compiled_instructions,
            accounts,
        )?;

        // Update the spending limits if present
        if !self.spending_limits.is_empty() {
            let current_timestamp = Clock::get()?.unix_timestamp;
            // Reset the spending limits if needed
            for spending_limit in &mut self.spending_limits {
                spending_limit.reset_if_needed(current_timestamp);
            }

            let tracked_pre_balances = check_pre_balances(smart_account_pubkey, accounts);
            // Execute the transaction message instructions one-by-one.
            // NOTE: `execute_message()` calls `self.to_instructions_and_accounts()`
            // which in turn calls `take()` on
            // `self.message.instructions`, therefore after this point no more
            // references or usages of `self.message` should be made to avoid
            // faulty behavior.
            executable_message.execute(smart_account_signer_seeds)?;
            // Evaluate the balance changes post-execution
            tracked_pre_balances.evaluate_balance_changes(&mut self.spending_limits)?;
        } else {
            // Execute the transaction message instructions one-by-one.
            // NOTE: `execute_message()` calls `self.to_instructions_and_accounts()`
            // which in turn calls `take()` on
            // `self.message.instructions`, therefore after this point no more
            // references or usages of `self.message` should be made to avoid
            // faulty behavior.
            executable_message.execute(smart_account_signer_seeds)?;
        }
        // Execute the post hook
        if let Some(post_hook) = &self.post_hook {
            post_hook.execute(
                self,
                post_hook_accounts,
                &settings_compiled_instructions,
                &accounts,
            )?;
        }

        Ok(())
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_constraint_u8_equals() {
        let constraint = DataConstraint {
            data_offset: 0,
            data_value: DataValue::U8(42),
            operator: DataOperator::Equals,
        };

        assert!(constraint.evaluate(&[42]).is_ok());

        assert_eq!(
            constraint.evaluate(&[41]).err().unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue.into()
        );
    }

    #[test]
    fn test_data_constraint_u8_greater_than() {
        let constraint = DataConstraint {
            data_offset: 0,
            data_value: DataValue::U8(10),
            operator: DataOperator::GreaterThan,
        };

        assert!(constraint.evaluate(&[11]).is_ok());

        assert_eq!(
            constraint.evaluate(&[10]).err().unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue.into()
        );
        assert_eq!(
            constraint.evaluate(&[9]).err().unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue.into()
        );
    }

    #[test]
    fn test_data_constraint_u16_little_endian() {
        let constraint = DataConstraint {
            data_offset: 1,
            data_value: DataValue::U16Le(0x1234),
            operator: DataOperator::Equals,
        };

        // Little endian: 0x1234 = [0x34, 0x12]
        assert!(constraint.evaluate(&[0x00, 0x34, 0x12]).is_ok());

        assert_eq!(
            constraint.evaluate(&[0x00, 0x12, 0x34]).err().unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue.into()
        );
    }

    #[test]
    fn test_data_constraint_u32_less_than_or_equal() {
        let constraint = DataConstraint {
            data_offset: 0,
            data_value: DataValue::U32Le(1000),
            operator: DataOperator::LessThanOrEqualTo,
        };

        // Little endian: 1000 = 0x03E8 = [0xE8, 0x03, 0x00, 0x00]
        assert!(constraint.evaluate(&[0xE8, 0x03, 0x00, 0x00]).is_ok()); // 1000
        assert!(constraint.evaluate(&[0xE7, 0x03, 0x00, 0x00]).is_ok()); // 999
        assert_eq!(
            constraint
                .evaluate(&[0xE9, 0x03, 0x00, 0x00])
                .err()
                .unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue.into()
        ); // 1001
    }

    #[test]
    fn test_data_constraint_u64_not_equals() {
        let constraint = DataConstraint {
            data_offset: 0,
            data_value: DataValue::U64Le(0x123456789ABCDEF0),
            operator: DataOperator::NotEquals,
        };

        let target_bytes = 0x123456789ABCDEF0u64.to_le_bytes();
        let different_bytes = 0x123456789ABCDEF1u64.to_le_bytes();

        assert_eq!(
            constraint.evaluate(&target_bytes).err().unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue.into()
        );
        assert!(constraint.evaluate(&different_bytes).is_ok());
    }

    #[test]
    fn test_data_constraint_u128_greater_than_or_equal() {
        let constraint = DataConstraint {
            data_offset: 0,
            data_value: DataValue::U128Le(1000),
            operator: DataOperator::GreaterThanOrEqualTo,
        };

        let equal_bytes = 1000u128.to_le_bytes();
        let greater_bytes = 1001u128.to_le_bytes();
        let lesser_bytes = 999u128.to_le_bytes();

        assert!(constraint.evaluate(&equal_bytes).is_ok());
        assert!(constraint.evaluate(&greater_bytes).is_ok());
        assert_eq!(
            constraint.evaluate(&lesser_bytes).err().unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue.into()
        );
    }

    #[test]
    fn test_data_constraint_u8_slice_equals() {
        let constraint = DataConstraint {
            data_offset: 8,
            data_value: DataValue::U8Slice(vec![0xDE, 0xAD, 0xBE, 0xEF]),
            operator: DataOperator::Equals,
        };

        let mut data = vec![0; 12];
        data[8..12].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

        assert!(constraint.evaluate(&data).is_ok());

        // Different bytes
        data[8] = 0xFF;
        assert_eq!(
            constraint.evaluate(&data).err().unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue.into()
        );
    }

    #[test]
    fn test_data_constraint_u8_slice_not_equals() {
        let constraint = DataConstraint {
            data_offset: 0,
            data_value: DataValue::U8Slice(vec![0x01, 0x02, 0x03]),
            operator: DataOperator::NotEquals,
        };

        assert!(constraint.evaluate(&[0x01, 0x02, 0x04]).is_ok());
        assert_eq!(
            constraint.evaluate(&[0x01, 0x02, 0x03]).err().unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue.into()
        );
    }

    #[test]
    fn test_data_constraint_u8_slice_invalid_operator() {
        let constraint = DataConstraint {
            data_offset: 0,
            data_value: DataValue::U8Slice(vec![0x01]),
            operator: DataOperator::GreaterThan, // Invalid for U8Slice
        };

        assert_eq!(
            constraint.evaluate(&[0x01]).err().unwrap(),
            SmartAccountError::ProgramInteractionUnsupportedSliceOperator.into()
        );
    }

    #[test]
    fn test_data_constraint_out_of_bounds() {
        let constraint = DataConstraint {
            data_offset: 5,
            data_value: DataValue::U8(42),
            operator: DataOperator::Equals,
        };

        // Data too short
        assert!(
            constraint.evaluate(&[1, 2, 3]).err().unwrap()
                == SmartAccountError::ProgramInteractionDataTooShort.into()
        );

        // Exact boundary
        assert_eq!(
            constraint.evaluate(&[1, 2, 3, 4, 5]).err().unwrap(),
            SmartAccountError::ProgramInteractionDataTooShort.into()
        );

        // Just enough data
        assert_eq!(
            constraint.evaluate(&[1, 2, 3, 4, 5, 41]).err().unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue.into()
        );
        assert!(constraint.evaluate(&[1, 2, 3, 4, 5, 42]).is_ok());
    }

    #[test]
    fn test_data_constraint_multi_byte_out_of_bounds() {
        let constraint = DataConstraint {
            data_offset: 2,
            data_value: DataValue::U32Le(1000),
            operator: DataOperator::Equals,
        };

        // Need 4 bytes starting at offset 2, so need at least 6 bytes total
        assert_eq!(
            constraint.evaluate(&[1, 2, 3, 4, 5]).err().unwrap(),
            SmartAccountError::ProgramInteractionDataTooShort.into()
        ); // Only 5 bytes

        let mut data = vec![0; 6];
        data[2..6].copy_from_slice(&1000u32.to_le_bytes());
        assert!(constraint.evaluate(&data).is_ok());
    }

    #[test]
    fn test_data_constraint_solana_instruction_discriminator() {
        // Simulate checking for a specific Solana instruction discriminator
        let swap_discriminator = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];

        let constraint = DataConstraint {
            data_offset: 0,
            data_value: DataValue::U8Slice(swap_discriminator.to_vec()),
            operator: DataOperator::Equals,
        };

        // Create instruction data with correct discriminator + some payload
        let mut instruction_data = swap_discriminator.to_vec();
        instruction_data.extend_from_slice(&[0xFF, 0xEE, 0xDD, 0xCC]); // Additional data

        assert!(constraint.evaluate(&instruction_data).is_ok());

        // Wrong discriminator
        let wrong_discriminator = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEE];
        let mut wrong_data = wrong_discriminator.to_vec();
        wrong_data.extend_from_slice(&[0xFF, 0xEE, 0xDD, 0xCC]);

        assert_eq!(
            constraint.evaluate(&wrong_data).err().unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue.into()
        );
    }

    #[test]
    fn test_data_constraint_amount_validation() {
        // Simulate validating a swap amount in instruction data
        // Amount at offset 12 (after 8-byte discriminator + 4-byte other data)
        let max_amount = 1000u64;

        let constraint = DataConstraint {
            data_offset: 12,
            data_value: DataValue::U64Le(max_amount),
            operator: DataOperator::LessThanOrEqualTo,
        };

        // Create instruction with amount = 500 (valid)
        let mut instruction_data = vec![0; 20]; // Discriminator + other data + amount
        instruction_data[12..20].copy_from_slice(&500u64.to_le_bytes());
        assert!(constraint.evaluate(&instruction_data).is_ok());

        // Create instruction with amount = 1500 (invalid)
        instruction_data[12..20].copy_from_slice(&1500u64.to_le_bytes());
        assert_eq!(
            constraint.evaluate(&instruction_data).err().unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue.into()
        );

        // Exactly at limit (valid)
        instruction_data[12..20].copy_from_slice(&1000u64.to_le_bytes());
        assert!(constraint.evaluate(&instruction_data).is_ok());
    }

    #[test]
    fn test_creation_payload_size_calculation() {
        let payload = ProgramInteractionPolicyCreationPayload {
            account_index: 1,
            pre_hook: None,
            post_hook: None,
            instructions_constraints: vec![InstructionConstraintCreationPayload {
                program_id: Pubkey::new_unique(),
                account_constraints: vec![AccountConstraintCreationPayload {
                    account_index: 0,
                    account_constraint: AccountConstraintTypeCreationPayload::Pubkey(vec![
                        Pubkey::new_unique(),
                        Pubkey::new_unique(),
                    ]),
                    owner: None,
                }],
                data_constraints: vec![
                    DataConstraint {
                        data_offset: 0,
                        data_value: DataValue::U8Slice(vec![
                            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF,
                        ]),
                        operator: DataOperator::Equals,
                    },
                    DataConstraint {
                        data_offset: 12,
                        data_value: DataValue::U64Le(1000),
                        operator: DataOperator::LessThanOrEqualTo,
                    },
                ],
            }],
            spending_limits: vec![LimitedSpendingLimit {
                mint: Pubkey::new_unique(),
                time_constraints: LimitedTimeConstraints {
                    start: 1640995200,            // Jan 1, 2022
                    expiration: Some(1672531200), // Jan 1, 2023
                    period: PeriodV2::Daily,
                },
                quantity_constraints: LimitedQuantityConstraints {
                    max_per_period: 1000,
                },
            }],
        };

        let calculated_size = payload.creation_payload_size();
        let actual_serialized = payload.try_to_vec().unwrap();
        let actual_size = actual_serialized.len();

        // Since InitSpace overestimates size, we only check that the calculated
        // size is greater than or equal to the actual size to make sure
        // serialization succeeds
        assert!(calculated_size >= actual_size);
    }

    #[test]
    fn test_policy_state_size_calculation() {
        let payload = ProgramInteractionPolicyCreationPayload {
            account_index: 1,
            pre_hook: None,
            post_hook: None,
            instructions_constraints: vec![InstructionConstraintCreationPayload {
                program_id: Pubkey::new_unique(),
                account_constraints: vec![AccountConstraintCreationPayload {
                    account_index: 0,
                    account_constraint: AccountConstraintTypeCreationPayload::Pubkey(vec![
                        Pubkey::new_unique(),
                        Pubkey::new_unique(),
                    ]),
                    owner: None,
                }],
                data_constraints: vec![
                    DataConstraint {
                        data_offset: 0,
                        data_value: DataValue::U8Slice(vec![
                            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF,
                        ]),
                        operator: DataOperator::Equals,
                    },
                    DataConstraint {
                        data_offset: 12,
                        data_value: DataValue::U64Le(1000),
                        operator: DataOperator::LessThanOrEqualTo,
                    },
                ],
            }],
            spending_limits: vec![LimitedSpendingLimit {
                mint: Pubkey::new_unique(),
                time_constraints: LimitedTimeConstraints {
                    start: 1640995200,
                    expiration: Some(1672531200),
                    period: PeriodV2::Daily,
                },
                quantity_constraints: LimitedQuantityConstraints {
                    max_per_period: 1000,
                },
            }],
        };

        let policy = payload.clone().to_policy_state().unwrap();
        let calculated_size = payload.policy_state_size();
        let actual_serialized = policy.try_to_vec().unwrap();
        let actual_size = actual_serialized.len();

        // Since InitSpace overestimates size, we only check that the calculated
        // size is greater than or equal to the actual size to make sure
        // serialization succeeds
        assert!(calculated_size >= actual_size);
    }

    #[test]
    fn test_pubkey_deduplication() {
        // Create shared pubkeys that will be reused
        let shared_program_id = Pubkey::new_unique();
        let shared_owner = Pubkey::new_unique();
        let shared_allowed_pubkey_1 = Pubkey::new_unique();
        let shared_allowed_pubkey_2 = Pubkey::new_unique();
        let unique_program_id = Pubkey::new_unique();

        let payload = ProgramInteractionPolicyCreationPayload {
            account_index: 0,
            instructions_constraints: vec![
                // First constraint uses shared_program_id
                InstructionConstraintCreationPayload {
                    program_id: shared_program_id,
                    account_constraints: vec![AccountConstraintCreationPayload {
                        account_index: 0,
                        account_constraint: AccountConstraintTypeCreationPayload::Pubkey(vec![
                            shared_allowed_pubkey_1,
                            shared_allowed_pubkey_2,
                        ]),
                        owner: Some(shared_owner),
                    }],
                    data_constraints: vec![],
                },
                // Second constraint reuses same program_id and pubkeys
                InstructionConstraintCreationPayload {
                    program_id: shared_program_id, // Duplicate
                    account_constraints: vec![AccountConstraintCreationPayload {
                        account_index: 1,
                        account_constraint: AccountConstraintTypeCreationPayload::Pubkey(vec![
                            shared_allowed_pubkey_1, // Duplicate
                            shared_allowed_pubkey_2, // Duplicate
                        ]),
                        owner: Some(shared_owner), // Duplicate
                    }],
                    data_constraints: vec![],
                },
                // Third constraint uses unique program_id
                InstructionConstraintCreationPayload {
                    program_id: unique_program_id, // New pubkey
                    account_constraints: vec![],
                    data_constraints: vec![],
                },
            ],
            pre_hook: Some(HookCreationPayload {
                num_extra_accounts: 2,
                program_id: shared_program_id, // Reuses shared_program_id
                account_constraints: vec![AccountConstraintCreationPayload {
                    account_index: 0,
                    account_constraint: AccountConstraintTypeCreationPayload::Pubkey(vec![
                        shared_allowed_pubkey_1, // Duplicate
                    ]),
                    owner: Some(shared_owner), // Duplicate
                }],
                instruction_data: vec![1, 2, 3],
                pass_inner_instructions: false,
            }),
            post_hook: Some(HookCreationPayload {
                num_extra_accounts: 1,
                program_id: shared_program_id, // Reuses shared_program_id
                account_constraints: vec![],
                instruction_data: vec![4, 5, 6],
                pass_inner_instructions: true,
            }),
            spending_limits: vec![],
        };

        let policy = payload.to_policy_state().unwrap();

        // Verify deduplication: Should only have 5 unique pubkeys in table
        // 1. shared_program_id
        // 2. shared_owner
        // 3. shared_allowed_pubkey_1
        // 4. shared_allowed_pubkey_2
        // 5. unique_program_id
        assert_eq!(
            policy.pubkey_table.len(),
            5,
            "Expected 5 unique pubkeys in table"
        );

        // Verify each shared pubkey appears exactly once
        assert_eq!(
            policy
                .pubkey_table
                .iter()
                .filter(|&pk| pk == &shared_program_id)
                .count(),
            1,
            "shared_program_id should appear exactly once"
        );
        assert_eq!(
            policy
                .pubkey_table
                .iter()
                .filter(|&pk| pk == &shared_owner)
                .count(),
            1,
            "shared_owner should appear exactly once"
        );
        assert_eq!(
            policy
                .pubkey_table
                .iter()
                .filter(|&pk| pk == &shared_allowed_pubkey_1)
                .count(),
            1,
            "shared_allowed_pubkey_1 should appear exactly once"
        );
        assert_eq!(
            policy
                .pubkey_table
                .iter()
                .filter(|&pk| pk == &shared_allowed_pubkey_2)
                .count(),
            1,
            "shared_allowed_pubkey_2 should appear exactly once"
        );
        assert_eq!(
            policy
                .pubkey_table
                .iter()
                .filter(|&pk| pk == &unique_program_id)
                .count(),
            1,
            "unique_program_id should appear exactly once"
        );

        // Verify that all instruction constraints reference correct indices
        for (i, constraint) in policy.instructions_constraints.iter().enumerate() {
            let resolved_program_id = policy
                .resolve_pubkey(constraint.program_id_index)
                .unwrap();
            if i < 2 {
                assert_eq!(
                    resolved_program_id, &shared_program_id,
                    "First two constraints should reference shared_program_id"
                );
            } else {
                assert_eq!(
                    resolved_program_id, &unique_program_id,
                    "Third constraint should reference unique_program_id"
                );
            }
        }

        // Verify hooks reference correct program_id
        let pre_hook = policy.pre_hook.as_ref().unwrap();
        assert_eq!(
            policy.resolve_pubkey(pre_hook.program_id_index).unwrap(),
            &shared_program_id,
            "Pre-hook should reference shared_program_id"
        );

        let post_hook = policy.post_hook.as_ref().unwrap();
        assert_eq!(
            policy.resolve_pubkey(post_hook.program_id_index).unwrap(),
            &shared_program_id,
            "Post-hook should reference shared_program_id"
        );
    }

    #[test]
    fn test_pubkey_deduplication_max_limit() {
        // Test that we properly enforce the 256 unique pubkey limit
        let mut instructions_constraints = Vec::new();

        // Create 255 unique program IDs (should succeed)
        for _ in 0..255 {
            instructions_constraints.push(InstructionConstraintCreationPayload {
                program_id: Pubkey::new_unique(),
                account_constraints: vec![],
                data_constraints: vec![],
            });
        }

        let payload = ProgramInteractionPolicyCreationPayload {
            account_index: 0,
            instructions_constraints: instructions_constraints.clone(),
            pre_hook: None,
            post_hook: None,
            spending_limits: vec![],
        };

        // Should succeed with 255 unique pubkeys
        assert!(payload.to_policy_state().is_ok());

        // Add one more to exceed limit
        instructions_constraints.push(InstructionConstraintCreationPayload {
            program_id: Pubkey::new_unique(),
            account_constraints: vec![],
            data_constraints: vec![],
        });

        let payload_over_limit = ProgramInteractionPolicyCreationPayload {
            account_index: 0,
            instructions_constraints,
            pre_hook: None,
            post_hook: None,
            spending_limits: vec![],
        };

        // Should fail with 256 unique pubkeys (exceeds u8 max)
        let result = payload_over_limit.to_policy_state();
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            SmartAccountError::ProgramInteractionTooManyUniquePubkeys
                .to_string()
        );
    }

    #[test]
    fn test_account_constraint_size_with_owner() {
        // Test size calculation with owner present
        let constraint_with_owner = AccountConstraint {
            account_index: 0,
            account_constraint: AccountConstraintType::Pubkey(vec![1, 2, 3]),
            owner_index: Some(5),
        };

        let expected_size = 1 + // account_index
            4 + // enum discriminator for account_constraint
            1 + // enum discriminator for Pubkey variant
            4 + // Vec length
            3 + // 3 indices
            1 + // Option discriminator
            1; // owner_index value

        assert_eq!(constraint_with_owner.size(), expected_size);

        // Test size calculation without owner
        let constraint_without_owner = AccountConstraint {
            account_index: 0,
            account_constraint: AccountConstraintType::Pubkey(vec![1, 2, 3]),
            owner_index: None,
        };

        let expected_size_no_owner = 1 + // account_index
            4 + // enum discriminator for account_constraint
            1 + // enum discriminator for Pubkey variant
            4 + // Vec length
            3 + // 3 indices
            1 + // Option discriminator
            0; // no owner_index value

        assert_eq!(constraint_without_owner.size(), expected_size_no_owner);

        // Verify the difference is exactly 1 byte
        assert_eq!(
            constraint_with_owner.size() - constraint_without_owner.size(),
            1
        );
    }
}
