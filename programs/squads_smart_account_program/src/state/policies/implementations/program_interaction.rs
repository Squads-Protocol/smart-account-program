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
    PolicyTrait, SmallVec, SmartAccountCompiledInstruction,
    SmartAccountSignerWrapper, TransactionMessage,
    TransactionPayload, TransactionPayloadDetails, HOOK_AUTHORITY_PUBKEY, SEED_EPHEMERAL_SIGNER,
    SEED_HOOK_AUTHORITY, SEED_PREFIX, SEED_SMART_ACCOUNT,
};
use anchor_lang::prelude::*;
use solana_program::{instruction::Instruction, pubkey};

// =============================================================================
// BUILTIN PUBKEY CONSTANTS
// =============================================================================

/// Wrapped SOL mint address
const WRAPPED_SOL: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

/// Starting index for builtin programs in the pubkey lookup table.
/// Indices 0-239 are for custom pubkeys stored in pubkey_table.
/// Indices 240-255 are reserved for commonly-used builtin programs.
const BUILTIN_INDEX_START: u8 = 240;

/// Resolve a builtin program pubkey by index (240-243).
/// Returns a reference to the static builtin pubkey constant.
///
/// Index mapping:
/// - 240: System Program
/// - 241: Token Program (SPL Token)
/// - 242: Associated Token Account Program
/// - 243: Token-2022 Program
/// - 244-255: Reserved for future use (currently invalid)
#[inline(always)]
fn resolve_builtin_pubkey(index: u8) -> Result<&'static Pubkey> {
    match index {
        240 => Ok(&anchor_lang::system_program::ID),
        241 => Ok(&anchor_spl::token::ID),
        242 => Ok(&anchor_spl::associated_token::ID),
        243 => Ok(&anchor_spl::token_2022::ID),
        244 => Ok(&anchor_spl::mint::USDC),
        245 => Ok(&WRAPPED_SOL),
        _ => Err(SmartAccountError::ProgramInteractionInvalidPubkeyTableIndex.into()),
    }
}

// =============================================================================
// CORE POLICY STRUCTURES
// =============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ProgramInteractionPolicy {
    /// The account index of the account that will be used to execute the policy
    pub account_index: u8,
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
    /// The program that this constraint applies to
    pub program_id: Pubkey,
    /// Account constraints (evaluated as logical AND)
    pub account_constraints: Vec<AccountConstraint>,
    /// Data constraints (evaluated as logical AND)
    pub data_constraints: Vec<DataConstraint>,
}

/// Compiled version of InstructionConstraint for use with pubkey_table
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct CompiledInstructionConstraint {
    /// Index into pubkey_table for the program_id
    pub program_id_index: u8,
    /// Account constraints (evaluated as logical AND)
    pub account_constraints: SmallVec<u8, CompiledAccountConstraint>,
    /// Data constraints (evaluated as logical AND)
    pub data_constraints: SmallVec<u8, DataConstraint>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct Hook {
    // Dictates how many extra accounts are required for the hook, beyond the program ID
    pub num_extra_accounts: u8,
    // Dictates constraints for the hook accounts
    pub account_constraints: Vec<AccountConstraint>,
    // Dictates which instruction data will be invoked
    pub instruction_data: Vec<u8>,
    // The program that will be invoked
    pub program_id: Pubkey,
    // Dictates if inner instruction data & account will be passed to the
    // instruction on top of the instruction data
    pub pass_inner_instructions: bool,
}

/// Compiled version of Hook for use with pubkey_table
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CompiledHook {
    // Dictates how many extra accounts are required for the hook, beyond the program ID
    pub num_extra_accounts: u8,
    // Dictates constraints for the hook accounts
    pub account_constraints: SmallVec<u8, CompiledAccountConstraint>,
    // Dictates which instruction data will be invoked
    pub instruction_data: SmallVec<u16, u8>,
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
        32 + // program_id
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

impl CompiledHook {
    pub fn size(&self) -> usize {
        1 + // num_accounts
        1 + self.account_constraints.iter().map(|c| c.size()).sum::<usize>() + // account_constraints
        2 + self.instruction_data.len() + // instruction_data
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
        32 + // program_id
        4 + self.account_constraints.iter().map(|c| c.size()).sum::<usize>() + // account_constraints vec
        4 + self.data_constraints.iter().map(|c| c.size()).sum::<usize>() // data_constraints vec
    }
}

impl CompiledInstructionConstraint {
    pub fn size(&self) -> usize {
        1 + // program_id_index
        1 + self.account_constraints.iter().map(|c| c.size()).sum::<usize>() + // account_constraints small_vec
        1 + self.data_constraints.iter().map(|c| c.size()).sum::<usize>() // data_constraints small_vec
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
    Pubkey(Vec<Pubkey>),
    AccountData(Vec<DataConstraint>),
}

impl AccountConstraintType {
    pub fn size(&self) -> usize {
        match self {
            AccountConstraintType::Pubkey(keys) => 1 + 4 + keys.len() * 32,
            AccountConstraintType::AccountData(constraints) => {
                1 + 4 + constraints.iter().map(|c| c.size()).sum::<usize>()
            }
        }
    }
}

/// Compiled version of AccountConstraintType for use with pubkey_table
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum CompiledAccountConstraintType {
    Pubkey(SmallVec<u8, u8>),  // Indices into pubkey_table
    AccountData(SmallVec<u8, DataConstraint>),
}

impl CompiledAccountConstraintType {
    pub fn size(&self) -> usize {
        match self {
            CompiledAccountConstraintType::Pubkey(indices) => 1 + 1 + indices.len(),
            CompiledAccountConstraintType::AccountData(constraints) => {
                1 + 1 + constraints.iter().map(|c| c.size()).sum::<usize>()
            }
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct AccountConstraint {
    pub account_index: u8,
    pub account_constraint: AccountConstraintType,
    pub owner: Option<Pubkey>,
}

/// Compiled version of AccountConstraint for use with pubkey_table
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct CompiledAccountConstraint {
    pub account_index: u8,
    pub account_constraint: CompiledAccountConstraintType,
    pub owner_index: Option<u8>,  // index into pubkey_table for owner
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
        self.account_constraint.size() + // account_constraint (enum, no vec prefix needed)
        1 + // option discriminator
        if self.owner.is_some() { 32 } else { 0 } // owner value (conditional)
    }
}

impl CompiledAccountConstraint {
    pub fn size(&self) -> usize {
        1 + // account_index
        self.account_constraint.size() + // account_constraint (includes enum discriminator + small_vec length + data)
        1 + // option discriminator
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

impl AccountConstraint {
    /// Evaluate the account constraint for a given set of instruction_account_indices and accounts
    pub fn evaluate_against_instruction_indices_and_accounts(
        &self,
        instruction_account_indices: &[u8],
        accounts: &[AccountInfo],
    ) -> Result<()> {
        // Get the account at the given constraint index
        let mapped_account_index = instruction_account_indices[self.account_index as usize];
        let account = &accounts[mapped_account_index as usize];

        self.evaluate_against_account_info(account)?;

        Ok(())
    }

    /// Simply evaluate the account constraint against a single AccountInfo
    pub fn evaluate_against_account_info(&self, account: &AccountInfo) -> Result<()> {
        // Evaluate the owner constraint
        if let Some(owner) = self.owner {
            require_eq!(
                account.owner,
                &owner,
                SmartAccountError::IllegalAccountOwner
            );
        };
        // Evaluate the account constraint
        match &self.account_constraint {
            AccountConstraintType::Pubkey(keys) => {
                if !keys.contains(&account.key) {
                    return Err(
                        SmartAccountError::ProgramInteractionAccountConstraintViolated.into(),
                    );
                }
            }
            AccountConstraintType::AccountData(constraints) => {
                let data = account.try_borrow_data()?;
                for constraint in constraints {
                    constraint.evaluate(&data)?;
                }
            }
        }
        Ok(())
    }

    /// Evaluate the account constraint against a set of AccountInfos
    pub fn evaluate_against_account_infos<'info>(
        &self,
        account_infos: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        let account_info_to_evalute = account_infos
            .get(self.account_index as usize)
            .ok_or(SmartAccountError::ProgramInteractionAccountConstraintViolated)
            .unwrap();

        self.evaluate_against_account_info(account_info_to_evalute)?;

        Ok(())
    }
}

// =============================================================================
// CREATION PAYLOAD TYPES
// =============================================================================

/// Limited subset of TimeConstraints
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct LimitedTimeConstraints {
    pub start: i64,
    pub expiration: Option<i64>,
    pub period: PeriodV2,
}

/// Limited subset of QuantityConstraints
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct LimitedQuantityConstraints {
    pub max_per_period: u64,
}

/// Limited subset of BalanceConstraint used to create a policy
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct LimitedSpendingLimit {
    pub mint: Pubkey,
    pub time_constraints: LimitedTimeConstraints,
    pub quantity_constraints: LimitedQuantityConstraints,
}

/// Compiled version of LimitedSpendingLimit for use with pubkey_table
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct CompiledLimitedSpendingLimit {
    pub mint_index: u8,
    pub time_constraints: LimitedTimeConstraints,
    pub quantity_constraints: LimitedQuantityConstraints,
}

/// Legacy payload used to create a program interaction policy (V1 format with embedded Pubkeys)
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ProgramInteractionPolicyCreationPayloadLegacy {
    pub account_index: u8,
    pub instructions_constraints: Vec<InstructionConstraint>,
    pub pre_hook: Option<Hook>,
    pub post_hook: Option<Hook>,
    pub spending_limits: Vec<LimitedSpendingLimit>,
}

/// Payload used to create a program interaction policy (V2 format with pubkey table and indices)
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ProgramInteractionPolicyCreationPayload {
    pub account_index: u8,
    pub pubkey_table: SmallVec<u8, Pubkey>,
    pub instructions_constraints: SmallVec<u8, CompiledInstructionConstraint>,
    pub pre_hook: Option<CompiledHook>,
    pub post_hook: Option<CompiledHook>,
    pub spending_limits: SmallVec<u8, CompiledLimitedSpendingLimit>,
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
    pub policy_signers: SmartAccountSignerWrapper,
}

// =============================================================================
// CORE POLICY IMPLEMENTATION
// =============================================================================

impl Hook {
    pub fn execute<'info>(
        &self,
        hook_accounts: &'info [AccountInfo<'info>],
        instructions: &[SmartAccountCompiledInstruction],
        instruction_accounts: &[AccountInfo<'info>],
    ) -> Result<()> {
        use borsh::BorshSerialize;

        // Evaluate the hook accounts
        for account_constraint in self.account_constraints.iter() {
            account_constraint.evaluate_against_account_infos(hook_accounts)?;
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
            require!(
                accounts[instruction.program_id_index as usize].key
                    == &instruction_constraint.program_id,
                SmartAccountError::ProgramInteractionProgramIdMismatch
            );

            // Evaluate the account constraints
            for account_constraint in &instruction_constraint.account_constraints {
                account_constraint.evaluate_against_instruction_indices_and_accounts(
                    &instruction.account_indexes,
                    accounts,
                )?;
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

impl CompiledLimitedSpendingLimit {
    pub fn size(&self) -> usize {
        1 + // mint_index
        self.time_constraints.size() + // time_constraints
        self.quantity_constraints.size() // quantity_constraints
    }
}

// =============================================================================
// PAYLOAD CONVERSION IMPLEMENTATIONS
// =============================================================================

impl PolicyPayloadConversionTrait for ProgramInteractionPolicyCreationPayloadLegacy {
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

        let mut spending_limits = self.spending_limits.clone();
        spending_limits.sort_by_key(|c| c.mint);

        let current_timestamp = Clock::get()?.unix_timestamp;

        Ok(ProgramInteractionPolicy {
            account_index: self.account_index,
            instructions_constraints: self.instructions_constraints,
            pre_hook: self.pre_hook,
            post_hook: self.post_hook,
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

impl ProgramInteractionPolicyCreationPayload {
    /// Resolve a pubkey from the table or builtin constants, checking bounds
    #[inline]
    fn resolve_pubkey(&self, index: u8) -> Result<Pubkey> {
        if index >= BUILTIN_INDEX_START {
            // Builtin program (indices 240-255)
            // resolve_builtin_pubkey handles validation internally
            Ok(*resolve_builtin_pubkey(index)?)
        } else {
            // Custom pubkey from table (indices 0-239)
            self.pubkey_table
                .get(index as usize)
                .copied()
                .ok_or_else(|| SmartAccountError::ProgramInteractionInvalidPubkeyTableIndex.into())
        }
    }

    /// Convert compiled constraint to full constraint
    fn expand_instruction_constraint(
        &self,
        compiled: &CompiledInstructionConstraint,
    ) -> Result<InstructionConstraint> {
        Ok(InstructionConstraint {
            program_id: self.resolve_pubkey(compiled.program_id_index)?,
            account_constraints: compiled.account_constraints
                .iter()
                .map(|ac| self.expand_account_constraint(ac))
                .collect::<Result<Vec<_>>>()?,
            data_constraints: compiled.data_constraints.to_vec(),
        })
    }

    fn expand_account_constraint(
        &self,
        compiled: &CompiledAccountConstraint,
    ) -> Result<AccountConstraint> {
        Ok(AccountConstraint {
            account_index: compiled.account_index,
            account_constraint: match &compiled.account_constraint {
                CompiledAccountConstraintType::Pubkey(indices) => {
                    // Pre-allocate with known capacity
                    let mut pubkeys = Vec::with_capacity(indices.len());
                    for &idx in indices.iter() {
                        pubkeys.push(self.resolve_pubkey(idx)?);
                    }
                    AccountConstraintType::Pubkey(pubkeys)
                }
                CompiledAccountConstraintType::AccountData(constraints) => {
                    AccountConstraintType::AccountData(constraints.to_vec())
                }
            },
            owner: compiled.owner_index
                .map(|idx| self.resolve_pubkey(idx))
                .transpose()?,
        })
    }

    fn expand_hook(&self, compiled: &CompiledHook) -> Result<Hook> {
        Ok(Hook {
            num_extra_accounts: compiled.num_extra_accounts,
            account_constraints: compiled.account_constraints
                .iter()
                .map(|ac| self.expand_account_constraint(ac))
                .collect::<Result<Vec<_>>>()?,
            instruction_data: compiled.instruction_data.to_vec(),
            program_id: self.resolve_pubkey(compiled.program_id_index)?,
            pass_inner_instructions: compiled.pass_inner_instructions,
        })
    }

    fn expand_spending_limit(
        &self,
        compiled: &CompiledLimitedSpendingLimit,
    ) -> Result<LimitedSpendingLimit> {
        Ok(LimitedSpendingLimit {
            mint: self.resolve_pubkey(compiled.mint_index)?,
            time_constraints: compiled.time_constraints.clone(),
            quantity_constraints: compiled.quantity_constraints.clone(),
        })
    }
}

impl PolicyPayloadConversionTrait for ProgramInteractionPolicyCreationPayload {
    type PolicyState = ProgramInteractionPolicy;

    fn to_policy_state(self) -> Result<ProgramInteractionPolicy> {
        // Validate limits
        require!(
            self.instructions_constraints.len() <= 20,
            SmartAccountError::ProgramInteractionTooManyInstructionConstraints
        );
        require!(
            self.spending_limits.len() <= 10,
            SmartAccountError::ProgramInteractionTooManySpendingLimits
        );
        require!(
            self.pubkey_table.len() <= 240,
            SmartAccountError::ProgramInteractionTooManyUniquePubkeys
        );

        // Expand indexed constraints to full Pubkey constraints
        let instructions_constraints = self.instructions_constraints
            .iter()
            .map(|ic| self.expand_instruction_constraint(ic))
            .collect::<Result<Vec<_>>>()?;

        let pre_hook = self.pre_hook
            .as_ref()
            .map(|h| self.expand_hook(h))
            .transpose()?;

        let post_hook = self.post_hook
            .as_ref()
            .map(|h| self.expand_hook(h))
            .transpose()?;

        // Expand indexed spending limits to full spending limits
        let mut spending_limits = self.spending_limits
            .iter()
            .map(|sl| self.expand_spending_limit(sl))
            .collect::<Result<Vec<_>>>()?;
        spending_limits.sort_by_key(|c| c.mint);

        let current_timestamp = Clock::get()?.unix_timestamp;

        Ok(ProgramInteractionPolicy {
            account_index: self.account_index,
            instructions_constraints,
            pre_hook,
            post_hook,
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
}

impl PolicySizeTrait for ProgramInteractionPolicyCreationPayloadLegacy {
    fn creation_payload_size(&self) -> usize {
        1 + // account_scope
        4 + self.instructions_constraints.iter().map(|c| c.size()).sum::<usize>() + // instructions_constraints vec
        1 + self.pre_hook.as_ref().map(|h| h.size()).unwrap_or(0) + // pre_hook
        1 + self.post_hook.as_ref().map(|h| h.size()).unwrap_or(0) + // post_hook
        4 + self.spending_limits.iter().map(|constraint| constraint.size()).sum::<usize>()
        // spending_limits vec
    }

    fn policy_state_size(&self) -> usize {
        1 + // account_index (account_scope becomes account_index in policy state)
        4 + self.instructions_constraints.iter().map(|c| c.size()).sum::<usize>() + // instructions_constraints vec
        1 + self.pre_hook.as_ref().map(|h| h.size()).unwrap_or(0) + // pre_hook
        1 + self.post_hook.as_ref().map(|h| h.size()).unwrap_or(0) + // post_hook
        4 + self.spending_limits.iter().map(|_| SpendingLimitV2::INIT_SPACE).sum::<usize>()
        // spending_limits vec
    }
}

impl PolicySizeTrait for ProgramInteractionPolicyCreationPayload {
    fn creation_payload_size(&self) -> usize {
        1 + // account_index
        1 + (self.pubkey_table.len() * 32) + // SmallVec<u8> + pubkey_table
        1 + self.instructions_constraints.iter().map(|c| c.size()).sum::<usize>() + // SmallVec<u8> + instructions_constraints
        1 + self.pre_hook.as_ref().map(|h| h.size()).unwrap_or(0) + // option + pre_hook
        1 + self.post_hook.as_ref().map(|h| h.size()).unwrap_or(0) + // option + post_hook
        1 + self.spending_limits.iter().map(|constraint| constraint.size()).sum::<usize>() // SmallVec<u8> + spending_limits
    }

    fn policy_state_size(&self) -> usize {
        // After expansion, state size is based on expanded Pubkeys, not indices
        // This is the same calculation as Legacy but we need to calculate the expanded size
        1 + // account_index
        4 + self.instructions_constraints.iter().map(|ic| {
            32 + // program_id (expanded from index)
            4 + ic.account_constraints.iter().map(|ac| {
                1 + // account_index
                match &ac.account_constraint {
                    CompiledAccountConstraintType::Pubkey(indices) => 1 + 4 + indices.len() * 32, // expanded
                    CompiledAccountConstraintType::AccountData(constraints) => {
                        1 + 4 + constraints.iter().map(|c| c.size()).sum::<usize>()
                    }
                } +
                1 + // owner Option discriminator
                if ac.owner_index.is_some() { 32 } else { 0 } // owner value (conditional)
            }).sum::<usize>() + // account_constraints vec
            4 + ic.data_constraints.iter().map(|c| c.size()).sum::<usize>() // data_constraints vec
        }).sum::<usize>() + // instructions_constraints vec
        1 + self.pre_hook.as_ref().map(|h| {
            1 + // num_accounts
            4 + h.account_constraints.iter().map(|ac| {
                1 + // account_index
                match &ac.account_constraint {
                    CompiledAccountConstraintType::Pubkey(indices) => 1 + 4 + indices.len() * 32,
                    CompiledAccountConstraintType::AccountData(constraints) => {
                        1 + 4 + constraints.iter().map(|c| c.size()).sum::<usize>()
                    }
                } +
                1 + // owner Option discriminator
                if ac.owner_index.is_some() { 32 } else { 0 } // owner value (conditional)
            }).sum::<usize>() +
            4 + h.instruction_data.len() +
            32 + // program_id (expanded from index)
            1 // pass_inner_instructions
        }).unwrap_or(0) + // pre_hook
        1 + self.post_hook.as_ref().map(|h| {
            1 + // num_accounts
            4 + h.account_constraints.iter().map(|ac| {
                1 + // account_index
                match &ac.account_constraint {
                    CompiledAccountConstraintType::Pubkey(indices) => 1 + 4 + indices.len() * 32,
                    CompiledAccountConstraintType::AccountData(constraints) => {
                        1 + 4 + constraints.iter().map(|c| c.size()).sum::<usize>()
                    }
                } +
                1 + // owner Option discriminator
                if ac.owner_index.is_some() { 32 } else { 0 } // owner value (conditional)
            }).sum::<usize>() +
            4 + h.instruction_data.len() +
            32 + // program_id (expanded from index)
            1 // pass_inner_instructions
        }).unwrap_or(0) + // post_hook
        4 + self.spending_limits.iter().map(|_| SpendingLimitV2::INIT_SPACE).sum::<usize>() // vec + spending_limits
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
    #[ignore = "This test doesn't work because `to_policy_state` uses the Clock"]
    fn test_creation_payload_size_calculation() {
        let payload = ProgramInteractionPolicyCreationPayloadLegacy {
            account_index: 1,
            pre_hook: None,
            post_hook: None,
            instructions_constraints: vec![InstructionConstraint {
                program_id: Pubkey::new_unique(),
                account_constraints: vec![AccountConstraint {
                    account_index: 0,
                    account_constraint: AccountConstraintType::Pubkey(vec![
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
    #[ignore = "This test doesn't work because `to_policy_state` uses the Clock"]
    fn test_policy_state_size_calculation() {
        let payload = ProgramInteractionPolicyCreationPayloadLegacy {
            account_index: 1,
            pre_hook: None,
            post_hook: None,
            instructions_constraints: vec![InstructionConstraint {
                program_id: Pubkey::new_unique(),
                account_constraints: vec![AccountConstraint {
                    account_index: 0,
                    account_constraint: AccountConstraintType::Pubkey(vec![
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
    fn test_resolve_pubkey() {
        let custom1 = Pubkey::new_unique();
        let custom2 = Pubkey::new_unique();
        let custom3 = Pubkey::new_unique();

        let payload = ProgramInteractionPolicyCreationPayload {
            account_index: 0,
            pubkey_table: SmallVec::from(vec![custom1, custom2, custom3]),
            instructions_constraints: SmallVec::from(vec![]),
            pre_hook: None,
            post_hook: None,
            spending_limits: SmallVec::from(vec![]),
        };

        // Test custom pubkey indices
        assert_eq!(payload.resolve_pubkey(0).unwrap(), custom1);
        assert_eq!(payload.resolve_pubkey(1).unwrap(), custom2);
        assert_eq!(payload.resolve_pubkey(2).unwrap(), custom3);

        // Test out of bounds
        assert!(payload.resolve_pubkey(3).is_err());
    }

    #[test]
    fn test_resolve_pubkey_with_builtins() {
        let payload = ProgramInteractionPolicyCreationPayload {
            account_index: 0,
            pubkey_table: SmallVec::from(vec![]),
            instructions_constraints: SmallVec::from(vec![]),
            pre_hook: None,
            post_hook: None,
            spending_limits: SmallVec::from(vec![]),
        };

        // Test all builtin program indices
        assert_eq!(payload.resolve_pubkey(240).unwrap(), anchor_lang::system_program::ID);
        assert_eq!(payload.resolve_pubkey(241).unwrap(), anchor_spl::token::ID);
        assert_eq!(payload.resolve_pubkey(242).unwrap(), anchor_spl::associated_token::ID);
        assert_eq!(payload.resolve_pubkey(243).unwrap(), anchor_spl::token_2022::ID);
        assert_eq!(payload.resolve_pubkey(244).unwrap(), anchor_spl::mint::USDC);
        assert_eq!(payload.resolve_pubkey(245).unwrap(), WRAPPED_SOL);

        // Test invalid builtin index
        assert!(payload.resolve_pubkey(246).is_err());
        assert!(payload.resolve_pubkey(255).is_err());
    }

    #[test]
    fn test_expand_instruction() {
        let custom1 = Pubkey::new_unique();
        let custom2 = Pubkey::new_unique();

        let payload = ProgramInteractionPolicyCreationPayload {
            account_index: 0,
            pubkey_table: SmallVec::from(vec![custom1, custom2]),
            instructions_constraints: SmallVec::from(vec![]),
            pre_hook: None,
            post_hook: None,
            spending_limits: SmallVec::from(vec![]),
        };

        let compiled = CompiledInstructionConstraint {
            program_id_index: 240, // System Program (builtin)
            account_constraints: SmallVec::from(vec![
                CompiledAccountConstraint {
                    account_index: 0,
                    account_constraint: CompiledAccountConstraintType::Pubkey(
                        SmallVec::from(vec![0, 241, 1]) // custom1, Token Program (builtin), custom2
                    ),
                    owner_index: Some(242), // Associated Token Program (builtin)
                },
                CompiledAccountConstraint {
                    account_index: 1,
                    account_constraint: CompiledAccountConstraintType::AccountData(
                        SmallVec::from(vec![
                            DataConstraint {
                                data_offset: 0,
                                data_value: DataValue::U8(42),
                                operator: DataOperator::Equals,
                            }
                        ])
                    ),
                    owner_index: None,
                },
            ]),
            data_constraints: SmallVec::from(vec![
                DataConstraint {
                    data_offset: 8,
                    data_value: DataValue::U64Le(1000),
                    operator: DataOperator::LessThanOrEqualTo,
                }
            ]),
        };

        let expanded = payload.expand_instruction_constraint(&compiled).unwrap();

        let expected = InstructionConstraint {
            program_id: anchor_lang::system_program::ID,
            account_constraints: vec![
                AccountConstraint {
                    account_index: 0,
                    account_constraint: AccountConstraintType::Pubkey(vec![
                        custom1,
                        anchor_spl::token::ID,
                        custom2,
                    ]),
                    owner: Some(anchor_spl::associated_token::ID),
                },
                AccountConstraint {
                    account_index: 1,
                    account_constraint: AccountConstraintType::AccountData(vec![
                        DataConstraint {
                            data_offset: 0,
                            data_value: DataValue::U8(42),
                            operator: DataOperator::Equals,
                        }
                    ]),
                    owner: None,
                },
            ],
            data_constraints: vec![
                DataConstraint {
                    data_offset: 8,
                    data_value: DataValue::U64Le(1000),
                    operator: DataOperator::LessThanOrEqualTo,
                }
            ],
        };

        assert_eq!(expanded, expected);
    }

    #[test]
    fn test_expand_hook() {
        let custom_program = Pubkey::new_unique();

        let payload = ProgramInteractionPolicyCreationPayload {
            account_index: 0,
            pubkey_table: SmallVec::from(vec![custom_program]),
            instructions_constraints: SmallVec::from(vec![]),
            pre_hook: None,
            post_hook: None,
            spending_limits: SmallVec::from(vec![]),
        };

        let compiled = CompiledHook {
            num_extra_accounts: 3,
            account_constraints: SmallVec::from(vec![
                CompiledAccountConstraint {
                    account_index: 0,
                    account_constraint: CompiledAccountConstraintType::Pubkey(
                        SmallVec::from(vec![240, 0]) // System Program (builtin), custom_program
                    ),
                    owner_index: Some(241), // Token Program (builtin)
                },
            ]),
            instruction_data: SmallVec::from(vec![1, 2, 3, 4, 5]),
            program_id_index: 243, // Token-2022 Program (builtin)
            pass_inner_instructions: true,
        };

        let expanded = payload.expand_hook(&compiled).unwrap();

        let expected = Hook {
            num_extra_accounts: 3,
            account_constraints: vec![
                AccountConstraint {
                    account_index: 0,
                    account_constraint: AccountConstraintType::Pubkey(vec![
                        anchor_lang::system_program::ID,
                        custom_program,
                    ]),
                    owner: Some(anchor_spl::token::ID),
                },
            ],
            instruction_data: vec![1, 2, 3, 4, 5],
            program_id: anchor_spl::token_2022::ID,
            pass_inner_instructions: true,
        };

        assert_eq!(expanded, expected);
    }

    #[test]
    fn test_expand_spending_limits() {
        let custom_mint = Pubkey::new_unique();

        let payload = ProgramInteractionPolicyCreationPayload {
            account_index: 0,
            pubkey_table: SmallVec::from(vec![custom_mint]),
            instructions_constraints: SmallVec::from(vec![]),
            pre_hook: None,
            post_hook: None,
            spending_limits: SmallVec::from(vec![]),
        };

        // Test with custom mint
        let compiled_custom = CompiledLimitedSpendingLimit {
            mint_index: 0, // custom_mint
            time_constraints: LimitedTimeConstraints {
                start: 1640995200,
                expiration: Some(1672531200),
                period: PeriodV2::Daily,
            },
            quantity_constraints: LimitedQuantityConstraints {
                max_per_period: 1000,
            },
        };

        let expanded_custom = payload.expand_spending_limit(&compiled_custom).unwrap();
        let expected_custom = LimitedSpendingLimit {
            mint: custom_mint,
            time_constraints: LimitedTimeConstraints {
                start: 1640995200,
                expiration: Some(1672531200),
                period: PeriodV2::Daily,
            },
            quantity_constraints: LimitedQuantityConstraints {
                max_per_period: 1000,
            },
        };
        assert_eq!(expanded_custom, expected_custom);

        // Test with USDC builtin
        let compiled_usdc = CompiledLimitedSpendingLimit {
            mint_index: 244, // USDC
            time_constraints: LimitedTimeConstraints {
                start: 1640995200,
                expiration: None,
                period: PeriodV2::Weekly,
            },
            quantity_constraints: LimitedQuantityConstraints {
                max_per_period: 5000,
            },
        };

        let expanded_usdc = payload.expand_spending_limit(&compiled_usdc).unwrap();
        let expected_usdc = LimitedSpendingLimit {
            mint: anchor_spl::mint::USDC,
            time_constraints: LimitedTimeConstraints {
                start: 1640995200,
                expiration: None,
                period: PeriodV2::Weekly,
            },
            quantity_constraints: LimitedQuantityConstraints {
                max_per_period: 5000,
            },
        };
        assert_eq!(expanded_usdc, expected_usdc);

        // Test with Wrapped SOL builtin
        let compiled_wsol = CompiledLimitedSpendingLimit {
            mint_index: 245, // Wrapped SOL
            time_constraints: LimitedTimeConstraints {
                start: 1640995200,
                expiration: Some(1704067200),
                period: PeriodV2::Monthly,
            },
            quantity_constraints: LimitedQuantityConstraints {
                max_per_period: 10000,
            },
        };

        let expanded_wsol = payload.expand_spending_limit(&compiled_wsol).unwrap();
        let expected_wsol = LimitedSpendingLimit {
            mint: WRAPPED_SOL,
            time_constraints: LimitedTimeConstraints {
                start: 1640995200,
                expiration: Some(1704067200),
                period: PeriodV2::Monthly,
            },
            quantity_constraints: LimitedQuantityConstraints {
                max_per_period: 10000,
            },
        };
        assert_eq!(expanded_wsol, expected_wsol);
    }
}
