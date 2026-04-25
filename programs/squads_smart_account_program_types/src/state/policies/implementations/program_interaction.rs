use solana_program::pubkey::Pubkey;

use crate::errors::SmartAccountError;
use crate::instructions::TransactionPayload;
use crate::state::policies::policy_core::PolicySizeTrait;
use crate::state::policies::utils::{
    PeriodV2, QuantityConstraints, SpendingLimitV2, TimeConstraints, UsageState,
};
use crate::state::SmartAccountSigner;

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Debug)]
pub struct ProgramInteractionPolicy {
    /// The account index of the account that will be used to execute the policy.
    pub account_index: u8,
    /// Constraints evaluated as a logical OR.
    pub instructions_constraints: Vec<InstructionConstraint>,
    /// Hook invoked before inner instruction execution.
    pub pre_hook: Option<Hook>,
    /// Hook invoked after inner instruction execution.
    pub post_hook: Option<Hook>,
    /// Spending limits applied during policy execution.
    pub spending_limits: Vec<SpendingLimitV2>,
}

impl ProgramInteractionPolicy {
    pub fn invariant(&self) -> Result<(), SmartAccountError> {
        let has_duplicate = self
            .spending_limits
            .windows(2)
            .any(|window| window[0].mint == window[1].mint);
        if has_duplicate {
            return Err(SmartAccountError::ProgramInteractionDuplicateSpendingLimit);
        }

        for spending_limit in &self.spending_limits {
            spending_limit.invariant()?;
        }

        Ok(())
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InstructionConstraint {
    pub program_id: Pubkey,
    pub account_constraints: Vec<AccountConstraint>,
    pub data_constraints: Vec<DataConstraint>,
}

impl InstructionConstraint {
    pub fn size(&self) -> usize {
        32 + 4
            + self
                .account_constraints
                .iter()
                .map(|c| c.size())
                .sum::<usize>()
            + 4
            + self
                .data_constraints
                .iter()
                .map(|c| c.size())
                .sum::<usize>()
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, Debug)]
pub struct Hook {
    pub num_extra_accounts: u8,
    pub account_constraints: Vec<AccountConstraint>,
    pub instruction_data: Vec<u8>,
    pub program_id: Pubkey,
    pub pass_inner_instructions: bool,
}

impl Hook {
    pub fn size(&self) -> usize {
        1 + 4
            + self
                .account_constraints
                .iter()
                .map(|c| c.size())
                .sum::<usize>()
            + 4
            + self.instruction_data.len()
            + 32
            + 1
    }

    pub fn num_accounts(&self) -> usize {
        self.num_extra_accounts
            .checked_add(1)
            .map(|v| v as usize)
            .unwrap()
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DataOperator {
    Equals,
    NotEquals,
    GreaterThan,
    GreaterThanOrEqualTo,
    LessThan,
    LessThanOrEqualTo,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DataValue {
    U8(u8),
    U16Le(u16),
    U32Le(u32),
    U64Le(u64),
    U128Le(u128),
    U8Slice(Vec<u8>),
}

impl DataValue {
    pub fn size(&self) -> usize {
        1 + match self {
            DataValue::U8(_) => 1,
            DataValue::U16Le(_) => 2,
            DataValue::U32Le(_) => 4,
            DataValue::U64Le(_) => 8,
            DataValue::U128Le(_) => 16,
            DataValue::U8Slice(bytes) => 4 + bytes.len(),
        }
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DataConstraint {
    pub data_offset: u64,
    pub data_value: DataValue,
    pub operator: DataOperator,
}

impl DataConstraint {
    pub fn size(&self) -> usize {
        8 + self.data_value.size() + 1
    }

    /// Evaluate constraint against instruction data (pure logic).
    pub fn evaluate(&self, data: &[u8]) -> Result<(), SmartAccountError> {
        let offset = self.data_offset as usize;

        let constraint_passed = match &self.data_value {
            DataValue::U8(expected) => {
                if offset >= data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort);
                }
                let actual = data[offset];
                self.compare(actual, *expected)
            }
            DataValue::U16Le(expected) => {
                if offset + 2 > data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort);
                }
                let bytes = &data[offset..offset + 2];
                let actual = u16::from_le_bytes([bytes[0], bytes[1]]);
                self.compare(actual, *expected)
            }
            DataValue::U32Le(expected) => {
                if offset + 4 > data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort);
                }
                let bytes = &data[offset..offset + 4];
                let actual = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                self.compare(actual, *expected)
            }
            DataValue::U64Le(expected) => {
                if offset + 8 > data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort);
                }
                let actual = u64::from_le_bytes(
                    data[offset..offset + 8]
                        .try_into()
                        .map_err(|_| SmartAccountError::ProgramInteractionDataParsingError)?,
                );
                self.compare(actual, *expected)
            }
            DataValue::U128Le(expected) => {
                if offset + 16 > data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort);
                }
                let actual = u128::from_le_bytes(
                    data[offset..offset + 16]
                        .try_into()
                        .map_err(|_| SmartAccountError::ProgramInteractionDataParsingError)?,
                );
                self.compare(actual, *expected)
            }
            DataValue::U8Slice(expected) => {
                if offset + expected.len() > data.len() {
                    return Err(SmartAccountError::ProgramInteractionDataTooShort);
                }
                let actual = &data[offset..offset + expected.len()];
                match self.operator {
                    DataOperator::Equals => actual == expected.as_slice(),
                    DataOperator::NotEquals => actual != expected.as_slice(),
                    _ => {
                        return Err(SmartAccountError::ProgramInteractionUnsupportedSliceOperator);
                    }
                }
            }
        };

        if constraint_passed {
            Ok(())
        } else {
            Err(SmartAccountError::ProgramInteractionInvalidNumericValue)
        }
    }

    fn compare<T: PartialOrd + PartialEq>(&self, actual: T, expected: T) -> bool {
        match self.operator {
            DataOperator::Equals => actual == expected,
            DataOperator::NotEquals => actual != expected,
            DataOperator::GreaterThan => actual > expected,
            DataOperator::GreaterThanOrEqualTo => actual >= expected,
            DataOperator::LessThan => actual < expected,
            DataOperator::LessThanOrEqualTo => actual <= expected,
        }
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq, Debug)]
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

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AccountConstraint {
    pub account_index: u8,
    pub account_constraint: AccountConstraintType,
    pub owner: Option<Pubkey>,
}

impl AccountConstraint {
    pub fn size(&self) -> usize {
        1 + 4 + self.account_constraint.size() + 32 + 1
    }
}

/// Limited subset of TimeConstraints.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq)]
pub struct LimitedTimeConstraints {
    pub start: i64,
    pub expiration: Option<i64>,
    pub period: PeriodV2,
}

impl LimitedTimeConstraints {
    pub fn size(&self) -> usize {
        8 + 1
            + match self.expiration {
                Some(_) => 8,
                None => 0,
            }
            + 1
    }
}

/// Limited subset of QuantityConstraints.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq)]
pub struct LimitedQuantityConstraints {
    pub max_per_period: u64,
}

impl LimitedQuantityConstraints {
    pub fn size(&self) -> usize {
        8
    }
}

/// Limited subset of spending limit used during creation.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq)]
pub struct LimitedSpendingLimit {
    pub mint: Pubkey,
    pub time_constraints: LimitedTimeConstraints,
    pub quantity_constraints: LimitedQuantityConstraints,
}

impl LimitedSpendingLimit {
    pub fn size(&self) -> usize {
        32 + self.time_constraints.size() + self.quantity_constraints.size()
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub struct ProgramInteractionPolicyCreationPayload {
    pub account_index: u8,
    pub instructions_constraints: Vec<InstructionConstraint>,
    pub pre_hook: Option<Hook>,
    pub post_hook: Option<Hook>,
    pub spending_limits: Vec<LimitedSpendingLimit>,
}

impl PolicySizeTrait for ProgramInteractionPolicyCreationPayload {
    fn creation_payload_size(&self) -> usize {
        1 + 4
            + self
                .instructions_constraints
                .iter()
                .map(|c| c.size())
                .sum::<usize>()
            + 1
            + self.pre_hook.as_ref().map(|h| h.size()).unwrap_or(0)
            + 1
            + self.post_hook.as_ref().map(|h| h.size()).unwrap_or(0)
            + 4
            + self.spending_limits.iter().map(|c| c.size()).sum::<usize>()
    }

    fn policy_state_size(&self) -> usize {
        1 + 4
            + self
                .instructions_constraints
                .iter()
                .map(|c| c.size())
                .sum::<usize>()
            + 1
            + self.pre_hook.as_ref().map(|h| h.size()).unwrap_or(0)
            + 1
            + self.post_hook.as_ref().map(|h| h.size()).unwrap_or(0)
            + 4
            + self
                .spending_limits
                .iter()
                .map(|_| SpendingLimitV2::INIT_SPACE)
                .sum::<usize>()
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub struct ProgramInteractionPayload {
    pub instruction_constraint_indices: Option<Vec<u8>>,
    pub transaction_payload: ProgramInteractionTransactionPayload,
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub enum ProgramInteractionTransactionPayload {
    AsyncTransaction(TransactionPayload),
    SyncTransaction(SyncTransactionPayloadDetails),
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub struct SyncTransactionPayloadDetails {
    pub account_index: u8,
    pub instructions: Vec<u8>,
}

impl ProgramInteractionTransactionPayload {
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
}

pub struct ProgramInteractionExecutionArgs {
    pub settings_key: Pubkey,
    pub transaction_key: Pubkey,
    pub proposal_key: Pubkey,
    pub policy_signers: Vec<SmartAccountSigner>,
}

// Suppress unused warnings when the types crate isn't compiled with all the
// extras (e.g. when some enums/constructors are unused by downstream code).
#[allow(dead_code)]
fn _period_touch(_: &PeriodV2, _: &TimeConstraints, _: &QuantityConstraints, _: &UsageState) {}

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
            SmartAccountError::ProgramInteractionInvalidNumericValue
        );
    }

    #[test]
    fn test_data_constraint_u16_little_endian() {
        let constraint = DataConstraint {
            data_offset: 1,
            data_value: DataValue::U16Le(0x1234),
            operator: DataOperator::Equals,
        };

        assert!(constraint.evaluate(&[0x00, 0x34, 0x12]).is_ok());
        assert_eq!(
            constraint.evaluate(&[0x00, 0x12, 0x34]).err().unwrap(),
            SmartAccountError::ProgramInteractionInvalidNumericValue
        );
    }

    #[test]
    fn test_data_constraint_u8_slice_invalid_operator() {
        let constraint = DataConstraint {
            data_offset: 0,
            data_value: DataValue::U8Slice(vec![0x01]),
            operator: DataOperator::GreaterThan,
        };

        assert_eq!(
            constraint.evaluate(&[0x01]).err().unwrap(),
            SmartAccountError::ProgramInteractionUnsupportedSliceOperator
        );
    }

    #[test]
    fn test_data_constraint_out_of_bounds() {
        let constraint = DataConstraint {
            data_offset: 5,
            data_value: DataValue::U8(42),
            operator: DataOperator::Equals,
        };

        assert_eq!(
            constraint.evaluate(&[1, 2, 3]).err().unwrap(),
            SmartAccountError::ProgramInteractionDataTooShort
        );
        assert!(constraint.evaluate(&[1, 2, 3, 4, 5, 42]).is_ok());
    }
}
