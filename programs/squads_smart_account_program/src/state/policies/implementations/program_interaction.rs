use crate::{
    errors::*,
    state::utils::{
        check_pre_balances, PeriodV2, QuantityConstraints, ResourceLimit, TimeConstraints,
        UsageState,
    },
    utils::{
        derive_ephemeral_signers, ExecutableTransactionMessage, SynchronousTransactionMessage,
    },
    CompiledInstruction, PolicyPayloadConversionTrait, PolicySizeTrait, PolicyTrait, SmallVec,
    SmartAccountCompiledInstruction, SmartAccountSigner, TransactionMessage, TransactionPayload,
    TransactionPayloadDetails, SEED_EPHEMERAL_SIGNER, SEED_PREFIX, SEED_SMART_ACCOUNT,
};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct ProgramInteractionPolicy {
    /// The account index of the account that will be used to execute the policy
    pub account_index: u8,
    // Constraints evaluated as a logical OR.
    pub instructions_constraints: Vec<InstructionConstraint>,
    pub resource_limits: Vec<ResourceLimit>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct InstructionConstraint {
    pub program_id: Pubkey,
    /// Constraints will be evaluated as a logical AND.
    pub account_constraints: Vec<AccountConstraint>,
    /// Constraints will be evaluated as a logical AND.
    pub data_constraints: Vec<DataConstraint>,
}

impl InstructionConstraint {
    pub fn size(&self) -> usize {
        32 + // program_id
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
impl DataConstraint {
    pub fn size(&self) -> usize {
        8 + // data_offset
        self.data_value.size() + // data_value
        1 // operator
    }
}

impl DataConstraint {
    /// Evaluate constraint against instruction data
    pub fn evaluate(&self, instruction_data: &[u8]) -> Result<()> {
        let offset = self.data_offset as usize;

        let constraint_passed = match &self.data_value {
            DataValue::U8(expected) => {
                // Check bounds
                if offset >= instruction_data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort.into());
                }
                let actual = instruction_data[offset];
                self.compare(actual, *expected)?
            }
            DataValue::U16Le(expected) => {
                // Check bounds for 2 bytes
                if offset + 2 > instruction_data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort.into());
                }
                let bytes = &instruction_data[offset..offset + 2];
                let actual = u16::from_le_bytes([bytes[0], bytes[1]]);
                self.compare(actual, *expected)?
            }
            DataValue::U32Le(expected) => {
                // Check bounds for 4 bytes
                if offset + 4 > instruction_data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort.into());
                }
                let bytes = &instruction_data[offset..offset + 4];
                let actual = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                self.compare(actual, *expected)?
            }
            DataValue::U64Le(expected) => {
                // Check bounds for 8 bytes
                if offset + 8 > instruction_data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort.into());
                }
                let actual = u64::from_le_bytes(
                    instruction_data[offset..offset + 8]
                        .try_into()
                        .map_err(|_| SmartAccountError::ProgramInteractionDataParsingError)?,
                );
                self.compare(actual, *expected)?
            }
            DataValue::U128Le(expected) => {
                // Check bounds for 16 bytes
                if offset + 16 > instruction_data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort.into());
                }
                let actual = u128::from_le_bytes(
                    instruction_data[offset..offset + 16]
                        .try_into()
                        .map_err(|_| SmartAccountError::ProgramInteractionDataParsingError)?,
                );
                self.compare(actual, *expected)?
            }
            DataValue::U8Slice(expected) => {
                // Check bounds for slice length
                if offset + expected.len() > instruction_data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort.into());
                }
                let actual = &instruction_data[offset..offset + expected.len()];
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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct AccountConstraint {
    pub account_index: u8,
    pub account_keys: Vec<Pubkey>,
}

impl AccountConstraint {
    pub fn size(&self) -> usize {
        1 + // account_index
        4 + self.account_keys.len() * 32 // account_keys vec
    }
}

impl AccountConstraint {
    /// Evaluate the account constraint for a given instruction
    pub fn evaluate(
        &self,
        instruction_account_indices: &[u8],
        accounts: &[AccountInfo],
    ) -> Result<()> {
        // Get the account at the given constraint index
        let mapped_account_index = instruction_account_indices[self.account_index as usize];
        let account = &accounts[mapped_account_index as usize];
        // Check if the account key is in the account keys
        if self.account_keys.contains(&account.key) {
            return Ok(());
        }
        Err(SmartAccountError::ProgramInteractionAccountConstraintViolated.into())
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
                account_constraint.evaluate(&instruction.account_indexes, accounts)?;
            }
            // Evaluate the data constraints
            for data_constraint in &instruction_constraint.data_constraints {
                data_constraint.evaluate(instruction.data.as_slice())?;
            }
        }
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
/// Limited subset of TimeConstraints.
pub struct LimitedTimeConstraints {
    pub start: i64,
    pub expiration: Option<i64>,
    pub period: PeriodV2,
}

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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
/// Limited subset of QuantityConstraints
pub struct LimitedQuantityConstraints {
    pub max_per_period: u64,
}

impl LimitedQuantityConstraints {
    pub fn size(&self) -> usize {
        8 // max_per_period
    }
}


/// Limited subset of BalanceConstraint used to create a policy
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct LimitedResourceLimit {
    pub mint: Pubkey,
    pub time_constraint: LimitedTimeConstraints,
    pub quantity_constraint: LimitedQuantityConstraints,
}

impl LimitedResourceLimit {
    pub fn size(&self) -> usize {
        32 + // mint
        self.time_constraint.size() + // time_constraint
        self.quantity_constraint.size() // quantity_constraint
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
/// Payload used to create a program interaction policy
pub struct ProgramInteractionPolicyCreationPayload {
    pub account_index: u8,
    pub instructions_constraints: Vec<InstructionConstraint>,
    pub resource_limits: Vec<LimitedResourceLimit>,
}

impl PolicyPayloadConversionTrait for ProgramInteractionPolicyCreationPayload {
    type PolicyState = ProgramInteractionPolicy;

    fn to_policy_state(self) -> ProgramInteractionPolicy {
        let mut resource_limits = self.resource_limits.clone();
        resource_limits.sort_by_key(|c| c.mint);

        ProgramInteractionPolicy {
            account_index: self.account_index,
            instructions_constraints: self.instructions_constraints,
            resource_limits: resource_limits
                .iter()
                .map(|resource_limit| ResourceLimit {
                    mint: resource_limit.mint,
                    time_constraints: TimeConstraints {
                        start: resource_limit.time_constraint.start,
                        period: resource_limit.time_constraint.period,
                        expiration: resource_limit.time_constraint.expiration,
                        accumulate_unused: false,
                    },
                    quantity_constraints: QuantityConstraints {
                        max_per_period: resource_limit.quantity_constraint.max_per_period,
                        max_per_use: 0,
                        enforce_exact_quantity: false,
                    },
                    usage: UsageState {
                        remaining_in_period: resource_limit.quantity_constraint.max_per_period,
                        last_reset: resource_limit.time_constraint.start,
                    },
                })
                .collect(),
        }
    }
}

impl PolicySizeTrait for ProgramInteractionPolicyCreationPayload {
    fn creation_payload_size(&self) -> usize {
        1 + // account_scope
        4 + self.instructions_constraints.iter().map(|c| c.size()).sum::<usize>() + // instructions_constraints vec
        4 + self.resource_limits.iter().map(|constraint| constraint.size()).sum::<usize>()
        // resource_limits vec
    }

    fn policy_state_size(&self) -> usize {
        1 + // account_index (account_scope becomes account_index in policy state)
        4 + self.instructions_constraints.iter().map(|c| c.size()).sum::<usize>() + // instructions_constraints vec
        4 + self.resource_limits.iter().map(|_| ResourceLimit::INIT_SPACE).sum::<usize>()
        // resource_limits vec
    }
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ProgramInteractionPayload {
    pub instruction_constraint_indices: Option<Vec<u8>>,
    pub transaction_payload: ProgramInteractionTransactionPayload,
}

impl ProgramInteractionPayload {
    pub fn get_transaction_payload(
        &self,
        settings_key: Pubkey,
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

    pub fn get_sync_transaction_payload(&self) -> Result<&SyncTransactionPayloadDetails> {
        match &self.transaction_payload {
            ProgramInteractionTransactionPayload::SyncTransaction(sync_transaction_payload) => {
                Ok(sync_transaction_payload)
            }
            _ => Err(SmartAccountError::InvalidPayload.into()),
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SyncTransactionPayloadDetails {
    pub account_index: u8,
    pub instructions: Vec<u8>,
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum ProgramInteractionTransactionPayload {
    AsyncTransaction(TransactionPayload),
    SyncTransaction(SyncTransactionPayloadDetails),
}

pub struct ProgramInteractionExecutionArgs {
    pub settings_key: Pubkey,
    pub transaction_key: Pubkey,
    pub proposal_key: Pubkey,
    pub policy_signers: Vec<SmartAccountSigner>,
}

impl PolicyTrait for ProgramInteractionPolicy {
    type PolicyState = Self;
    type CreationPayload = ProgramInteractionPolicyCreationPayload;
    type UsagePayload = ProgramInteractionPayload;
    type ExecutionArgs = ProgramInteractionExecutionArgs;

    fn invariant(&self) -> Result<()> {
        // There can't be duplicate balance constraint for the same mint
        // Assumes that the balance constraints are sorted by mint
        let has_duplicate = self
            .resource_limits
            .windows(2)
            .any(|window| window[0].mint == window[1].mint);
        require!(!has_duplicate, SmartAccountError::ProgramInteractionDuplicateResourceLimit);

        // Each resource limits invariant must be valid
        for resource_limit in &self.resource_limits {
            resource_limit.invariant()?;
        }

        Ok(())
    }

    fn validate_payload(&self, payload: &Self::UsagePayload) -> Result<()> {
        let payload_account_index = match &payload.transaction_payload {
            ProgramInteractionTransactionPayload::AsyncTransaction(transaction_payload) => {
                transaction_payload.account_index
            }
            ProgramInteractionTransactionPayload::SyncTransaction(sync_transaction_payload) => {
                sync_transaction_payload.account_index
            }
        };
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
                self.execute_payload(args, payload, accounts)
            }
            ProgramInteractionTransactionPayload::SyncTransaction(..) => {
                self.execute_payload_sync(args, payload, accounts)
            }
        }
    }
}

impl ProgramInteractionPolicy {
    fn execute_payload<'info>(
        &mut self,
        args: ProgramInteractionExecutionArgs,
        payload: &ProgramInteractionPayload,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        let transaction_payload =
            payload.get_transaction_payload(args.settings_key, args.transaction_key)?;
        // Evaluate the instruction constraints
        if let Some(instruction_constraint_indices) = &payload.instruction_constraint_indices {
            self.evaluate_instruction_constraints(
                instruction_constraint_indices,
                &transaction_payload.message.instructions,
                accounts,
            )?;
        }

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
        let num_lookups = transaction_payload.message.address_table_lookups.len();

        let message_account_infos = accounts
            .get(num_lookups..)
            .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;
        let address_lookup_table_account_infos = accounts
            .get(..num_lookups)
            .ok_or(SmartAccountError::InvalidNumberOfAccounts)?;

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

        // Update the resource limits if present
        if !self.resource_limits.is_empty() {
            let current_timestamp = Clock::get()?.unix_timestamp;
            // Reset the resource limits if needed
            for resource_limit in &mut self.resource_limits {
                resource_limit.reset_if_needed(current_timestamp);
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
            tracked_pre_balances.evaluate_balance_changes(&mut self.resource_limits)?;
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
        Ok(())
    }

    // Synchronous variant of execute_payload
    fn execute_payload_sync<'info>(
        &mut self,
        args: ProgramInteractionExecutionArgs,
        payload: &ProgramInteractionPayload,
        accounts: &'info [AccountInfo<'info>],
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

        let executable_message = SynchronousTransactionMessage::new_validated(
            &settings_key,
            &smart_account_pubkey,
            &args.policy_signers,
            settings_compiled_instructions,
            accounts,
        )?;

        // Update the resource limits if present
        if !self.resource_limits.is_empty() {
            let current_timestamp = Clock::get()?.unix_timestamp;
            // Reset the resource limits if needed
            for resource_limit in &mut self.resource_limits {
                resource_limit.reset_if_needed(current_timestamp);
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
            tracked_pre_balances.evaluate_balance_changes(&mut self.resource_limits)?;
        } else {
            // Execute the transaction message instructions one-by-one.
            // NOTE: `execute_message()` calls `self.to_instructions_and_accounts()`
            // which in turn calls `take()` on
            // `self.message.instructions`, therefore after this point no more
            // references or usages of `self.message` should be made to avoid
            // faulty behavior.
            executable_message.execute(smart_account_signer_seeds)?;
        }

        Ok(())
    }
}

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
            instructions_constraints: vec![InstructionConstraint {
                program_id: Pubkey::new_unique(),
                account_constraints: vec![AccountConstraint {
                    account_index: 0,
                    account_keys: vec![Pubkey::new_unique(), Pubkey::new_unique()],
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
            resource_limits: vec![LimitedResourceLimit {
                mint: Pubkey::new_unique(),
                time_constraint: LimitedTimeConstraints {
                    start: 1640995200,            // Jan 1, 2022
                    expiration: Some(1672531200), // Jan 1, 2023
                    period: PeriodV2::Day,
                },
                quantity_constraint: LimitedQuantityConstraints {
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
            instructions_constraints: vec![InstructionConstraint {
                program_id: Pubkey::new_unique(),
                account_constraints: vec![AccountConstraint {
                    account_index: 0,
                    account_keys: vec![Pubkey::new_unique(), Pubkey::new_unique()],
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
            resource_limits: vec![LimitedResourceLimit {
                mint: Pubkey::new_unique(),
                time_constraint: LimitedTimeConstraints {
                    start: 1640995200,
                    expiration: Some(1672531200),
                    period: PeriodV2::Day,
                },
                quantity_constraint: LimitedQuantityConstraints {
                    max_per_period: 1000,
                },
            }],
        };

        let policy = payload.clone().to_policy_state();
        let calculated_size = payload.policy_state_size();
        let actual_serialized = policy.try_to_vec().unwrap();
        let actual_size = actual_serialized.len();

        // Since InitSpace overestimates size, we only check that the calculated
        // size is greater than or equal to the actual size to make sure
        // serialization succeeds
        assert!(calculated_size >= actual_size);
    }
}
