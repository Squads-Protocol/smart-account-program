//! Error codes produced by the Squads Smart Account Program.
//!
//! This enum mirrors the program's anchor `#[error_code]` enum with identical
//! numeric codes (anchor assigns sequential `u32` codes starting at 6000). Off-chain
//! consumers can use `TryFrom<u32>` to decode error codes returned by failed
//! transactions without pulling in `anchor-lang`.

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[repr(u32)]
pub enum SmartAccountError {
    #[error("Account is not empty")]
    AccountNotEmpty = 6000,
    #[error("Found multiple signers with the same pubkey")]
    DuplicateSigner = 6001,
    #[error("Signers array is empty")]
    EmptySigners = 6002,
    #[error("Too many signers, can be up to 65535")]
    TooManySigners = 6003,
    #[error("Invalid threshold, must be between 1 and number of signers with vote permissions")]
    InvalidThreshold = 6004,
    #[error("Attempted to perform an unauthorized action")]
    Unauthorized = 6005,
    #[error("Provided pubkey is not a signer of the smart account")]
    NotASigner = 6006,
    #[error("TransactionMessage is malformed.")]
    InvalidTransactionMessage = 6007,
    #[error("Proposal is stale")]
    StaleProposal = 6008,
    #[error("Invalid proposal status")]
    InvalidProposalStatus = 6009,
    #[error("Invalid transaction index")]
    InvalidTransactionIndex = 6010,
    #[error("Signer already approved the transaction")]
    AlreadyApproved = 6011,
    #[error("Signer already rejected the transaction")]
    AlreadyRejected = 6012,
    #[error("Signer already cancelled the transaction")]
    AlreadyCancelled = 6013,
    #[error("Wrong number of accounts provided")]
    InvalidNumberOfAccounts = 6014,
    #[error("Invalid account provided")]
    InvalidAccount = 6015,
    #[error("Cannot remove last signer")]
    RemoveLastSigner = 6016,
    #[error("Signers don't include any voters")]
    NoVoters = 6017,
    #[error("Signers don't include any proposers")]
    NoProposers = 6018,
    #[error("Signers don't include any executors")]
    NoExecutors = 6019,
    #[error("`stale_transaction_index` must be <= `transaction_index`")]
    InvalidStaleTransactionIndex = 6020,
    #[error("Instruction not supported for controlled smart account")]
    NotSupportedForControlled = 6021,
    #[error("Proposal time lock has not been released")]
    TimeLockNotReleased = 6022,
    #[error("Config transaction must have at least one action")]
    NoActions = 6023,
    #[error("Missing account")]
    MissingAccount = 6024,
    #[error("Invalid mint")]
    InvalidMint = 6025,
    #[error("Invalid destination")]
    InvalidDestination = 6026,
    #[error("Spending limit exceeded")]
    SpendingLimitExceeded = 6027,
    #[error("Decimals don't match the mint")]
    DecimalsMismatch = 6028,
    #[error("Signer has unknown permission")]
    UnknownPermission = 6029,
    #[error("Account is protected, it cannot be passed into a CPI as writable")]
    ProtectedAccount = 6030,
    #[error("Time lock exceeds the maximum allowed (90 days)")]
    TimeLockExceedsMaxAllowed = 6031,
    #[error("Account is not owned by Smart Account program")]
    IllegalAccountOwner = 6032,
    #[error("Rent reclamation is disabled for this smart account")]
    RentReclamationDisabled = 6033,
    #[error("Invalid rent collector address")]
    InvalidRentCollector = 6034,
    #[error("Proposal is for another smart account")]
    ProposalForAnotherSmartAccount = 6035,
    #[error("Transaction is for another smart account")]
    TransactionForAnotherSmartAccount = 6036,
    #[error("Transaction doesn't match proposal")]
    TransactionNotMatchingProposal = 6037,
    #[error("Transaction is not last in batch")]
    TransactionNotLastInBatch = 6038,
    #[error("Batch is not empty")]
    BatchNotEmpty = 6039,
    #[error("Invalid SpendingLimit amount")]
    SpendingLimitInvalidAmount = 6040,
    #[error("Invalid Instruction Arguments")]
    InvalidInstructionArgs = 6041,
    #[error("Final message buffer hash doesnt match the expected hash")]
    FinalBufferHashMismatch = 6042,
    #[error("Final buffer size cannot exceed 4000 bytes")]
    FinalBufferSizeExceeded = 6043,
    #[error("Final buffer size mismatch")]
    FinalBufferSizeMismatch = 6044,
    #[error("smart_account_create has been deprecated. Use smart_account_create_v2 instead.")]
    SmartAccountCreateDeprecated = 6045,
    #[error("Signers do not reach consensus threshold")]
    ThresholdNotReached = 6046,
    #[error("Invalid number of signer accounts. Must be greater or equal to the threshold")]
    InvalidSignerCount = 6047,
    #[error("Missing signature")]
    MissingSignature = 6048,
    #[error("Insufficient aggregate permissions across signing members")]
    InsufficientAggregatePermissions = 6049,
    #[error("Insufficient vote permissions across signing members")]
    InsufficientVotePermissions = 6050,
    #[error("Smart account must not be time locked")]
    TimeLockNotZero = 6051,
    #[error("Feature not implemented")]
    NotImplemented = 6052,
    #[error("Invalid cadence configuration")]
    SpendingLimitInvalidCadenceConfiguration = 6053,
    #[error("Invalid data constraint")]
    InvalidDataConstraint = 6054,
    #[error("Invalid payload")]
    InvalidPayload = 6055,
    #[error("Protected instruction")]
    ProtectedInstruction = 6056,
    #[error("Placeholder error")]
    PlaceholderError = 6057,
    #[error("Invalid policy payload")]
    InvalidPolicyPayload = 6058,
    #[error("Invalid empty policy")]
    InvalidEmptyPolicy = 6059,
    #[error("Transaction is for another policy")]
    TransactionForAnotherPolicy = 6060,
    #[error("Program interaction sync payload not allowed with async transaction")]
    ProgramInteractionAsyncPayloadNotAllowedWithSyncTransaction = 6061,
    #[error("Program interaction sync payload not allowed with sync transaction")]
    ProgramInteractionSyncPayloadNotAllowedWithAsyncTransaction = 6062,
    #[error("Program interaction data constraint failed: instruction data too short")]
    ProgramInteractionDataTooShort = 6063,
    #[error("Program interaction data constraint failed: invalid numeric value")]
    ProgramInteractionInvalidNumericValue = 6064,
    #[error("Program interaction data constraint failed: invalid byte sequence")]
    ProgramInteractionInvalidByteSequence = 6065,
    #[error("Program interaction data constraint failed: unsupported operator for byte slice")]
    ProgramInteractionUnsupportedSliceOperator = 6066,
    #[error("Program interaction constraint failed: instruction data parsing error")]
    ProgramInteractionDataParsingError = 6067,
    #[error("Program interaction constraint failed: program ID mismatch")]
    ProgramInteractionProgramIdMismatch = 6068,
    #[error("Program interaction constraint violation: account constraint")]
    ProgramInteractionAccountConstraintViolated = 6069,
    #[error("Program interaction constraint violation: instruction constraint index out of bounds")]
    ProgramInteractionConstraintIndexOutOfBounds = 6070,
    #[error("Program interaction constraint violation: instruction count mismatch")]
    ProgramInteractionInstructionCountMismatch = 6071,
    #[error("Program interaction constraint violation: insufficient remaining lamport allowance")]
    ProgramInteractionInsufficientLamportAllowance = 6072,
    #[error("Program interaction constraint violation: insufficient remaining token allowance")]
    ProgramInteractionInsufficientTokenAllowance = 6073,
    #[error("Program interaction constraint violation: modified illegal balance")]
    ProgramInteractionModifiedIllegalBalance = 6074,
    #[error("Program interaction constraint violation: illegal token account modification")]
    ProgramInteractionIllegalTokenAccountModification = 6075,
    #[error("Program interaction invariant violation: duplicate spending limit for the same mint")]
    ProgramInteractionDuplicateSpendingLimit = 6076,
    #[error("Program interaction constraint violation: too many instruction constraints. Max is 20")]
    ProgramInteractionTooManyInstructionConstraints = 6077,
    #[error("Program interaction constraint violation: too many spending limits. Max is 10")]
    ProgramInteractionTooManySpendingLimits = 6078,
    #[error("Program interaction hook violation: template hook error")]
    ProgramInteractionTemplateHookError = 6079,
    #[error("Program interaction hook violation: hook authority cannot be part of hook accounts")]
    ProgramInteractionHookAuthorityCannotBePartOfHookAccounts = 6080,
    #[error("Spending limit is not active")]
    SpendingLimitNotActive = 6081,
    #[error("Spending limit is expired")]
    SpendingLimitExpired = 6082,
    #[error("Spending limit policy invariant violation: usage state cannot be Some() if accumulate_unused is true")]
    SpendingLimitPolicyInvariantAccumulateUnused = 6083,
    #[error("Amount violates exact quantity constraint")]
    SpendingLimitViolatesExactQuantityConstraint = 6084,
    #[error("Amount violates max per use constraint")]
    SpendingLimitViolatesMaxPerUseConstraint = 6085,
    #[error("Spending limit is insufficient")]
    SpendingLimitInsufficientRemainingAmount = 6086,
    #[error("Spending limit invariant violation: max per period must be non-zero")]
    SpendingLimitInvariantMaxPerPeriodZero = 6087,
    #[error("Spending limit invariant violation: start time must be positive")]
    SpendingLimitInvariantStartTimePositive = 6088,
    #[error("Spending limit invariant violation: expiration must be greater than start")]
    SpendingLimitInvariantExpirationSmallerThanStart = 6089,
    #[error("Spending limit invariant violation: overflow enabled must have expiration")]
    SpendingLimitInvariantOverflowEnabledMustHaveExpiration = 6090,
    #[error("Spending limit invariant violation: one time period cannot have overflow enabled")]
    SpendingLimitInvariantOneTimePeriodCannotHaveOverflowEnabled = 6091,
    #[error("Spending limit invariant violation: remaining amount must be less than max amount")]
    SpendingLimitInvariantOverflowRemainingAmountGreaterThanMaxAmount = 6092,
    #[error("Spending limit invariant violation: remaining amount must be less than or equal to max per period")]
    SpendingLimitInvariantRemainingAmountGreaterThanMaxPerPeriod = 6093,
    #[error("Spending limit invariant violation: exact quantity must have max per use non-zero")]
    SpendingLimitInvariantExactQuantityMaxPerUseZero = 6094,
    #[error("Spending limit invariant violation: max per use must be less than or equal to max per period")]
    SpendingLimitInvariantMaxPerUseGreaterThanMaxPerPeriod = 6095,
    #[error("Spending limit invariant violation: custom period must be positive")]
    SpendingLimitInvariantCustomPeriodNegative = 6096,
    #[error("Spending limit policy invariant violation: cannot have duplicate destinations for the same mint")]
    SpendingLimitPolicyInvariantDuplicateDestinations = 6097,
    #[error("Spending limit invariant violation: last reset must be between start and expiration")]
    SpendingLimitInvariantLastResetOutOfBounds = 6098,
    #[error("Spending limit invariant violation: last reset must be greater than start")]
    SpendingLimitInvariantLastResetSmallerThanStart = 6099,
    #[error("Internal fund transfer policy invariant violation: source account index is not allowed")]
    InternalFundTransferPolicyInvariantSourceAccountIndexNotAllowed = 6100,
    #[error("Internal fund transfer policy invariant violation: destination account index is not allowed")]
    InternalFundTransferPolicyInvariantDestinationAccountIndexNotAllowed = 6101,
    #[error("Internal fund transfer policy invariant violation: source and destination cannot be the same")]
    InternalFundTransferPolicyInvariantSourceAndDestinationCannotBeTheSame = 6102,
    #[error("Internal fund transfer policy invariant violation: mint is not allowed")]
    InternalFundTransferPolicyInvariantMintNotAllowed = 6103,
    #[error("Internal fund transfer policy invariant violation: amount must be greater than 0")]
    InternalFundTransferPolicyInvariantAmountZero = 6104,
    #[error("Internal fund transfer policy invariant violation: cannot have duplicate mints")]
    InternalFundTransferPolicyInvariantDuplicateMints = 6105,
    #[error("Consensus account is not a settings")]
    ConsensusAccountNotSettings = 6106,
    #[error("Consensus account is not a policy")]
    ConsensusAccountNotPolicy = 6107,
    #[error("Settings change policy invariant violation: actions must be non-zero")]
    SettingsChangePolicyActionsMustBeNonZero = 6108,
    #[error("Settings change policy violation: submitted settings account must match policy settings key")]
    SettingsChangeInvalidSettingsKey = 6109,
    #[error("Settings change policy violation: submitted settings account must be writable")]
    SettingsChangeInvalidSettingsAccount = 6110,
    #[error("Settings change policy violation: rent payer must be writable and signer")]
    SettingsChangeInvalidRentPayer = 6111,
    #[error("Settings change policy violation: system program must be the system program")]
    SettingsChangeInvalidSystemProgram = 6112,
    #[error("Settings change policy violation: signer does not match allowed signer")]
    SettingsChangeAddSignerViolation = 6113,
    #[error("Settings change policy violation: signer permissions does not match allowed signer permissions")]
    SettingsChangeAddSignerPermissionsViolation = 6114,
    #[error("Settings change policy violation: signer removal does not mach allowed signer removal")]
    SettingsChangeRemoveSignerViolation = 6115,
    #[error("Settings change policy violation: time lock does not match allowed time lock")]
    SettingsChangeChangeTimelockViolation = 6116,
    #[error("Settings change policy violation: action does not match allowed action")]
    SettingsChangeActionMismatch = 6117,
    #[error("Settings change policy invariant violation: cannot have duplicate actions")]
    SettingsChangePolicyInvariantDuplicateActions = 6118,
    #[error("Settings change policy invariant violation: action indices must match actions length")]
    SettingsChangePolicyInvariantActionIndicesActionsLengthMismatch = 6119,
    #[error("Settings change policy invariant violation: action index out of bounds")]
    SettingsChangePolicyInvariantActionIndexOutOfBounds = 6120,
    #[error("Policy is not active yet")]
    PolicyNotActiveYet = 6121,
    #[error("Policy invariant violation: invalid policy expiration")]
    PolicyInvariantInvalidExpiration = 6122,
    #[error("Policy expiration violation: submitted settings key does not match policy settings key")]
    PolicyExpirationViolationPolicySettingsKeyMismatch = 6123,
    #[error("Policy expiration violation: state expiration requires the settings to be submitted")]
    PolicyExpirationViolationSettingsAccountNotPresent = 6124,
    #[error("Policy expiration violation: state hash has expired")]
    PolicyExpirationViolationHashExpired = 6125,
    #[error("Policy expiration violation: timestamp has expired")]
    PolicyExpirationViolationTimestampExpired = 6126,
}

impl SmartAccountError {
    pub const fn code(self) -> u32 {
        self as u32
    }
}

impl TryFrom<u32> for SmartAccountError {
    type Error = ();
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            6000 => Ok(Self::AccountNotEmpty),
            6001 => Ok(Self::DuplicateSigner),
            6002 => Ok(Self::EmptySigners),
            6003 => Ok(Self::TooManySigners),
            6004 => Ok(Self::InvalidThreshold),
            6005 => Ok(Self::Unauthorized),
            6006 => Ok(Self::NotASigner),
            6007 => Ok(Self::InvalidTransactionMessage),
            6008 => Ok(Self::StaleProposal),
            6009 => Ok(Self::InvalidProposalStatus),
            6010 => Ok(Self::InvalidTransactionIndex),
            6011 => Ok(Self::AlreadyApproved),
            6012 => Ok(Self::AlreadyRejected),
            6013 => Ok(Self::AlreadyCancelled),
            6014 => Ok(Self::InvalidNumberOfAccounts),
            6015 => Ok(Self::InvalidAccount),
            6016 => Ok(Self::RemoveLastSigner),
            6017 => Ok(Self::NoVoters),
            6018 => Ok(Self::NoProposers),
            6019 => Ok(Self::NoExecutors),
            6020 => Ok(Self::InvalidStaleTransactionIndex),
            6021 => Ok(Self::NotSupportedForControlled),
            6022 => Ok(Self::TimeLockNotReleased),
            6023 => Ok(Self::NoActions),
            6024 => Ok(Self::MissingAccount),
            6025 => Ok(Self::InvalidMint),
            6026 => Ok(Self::InvalidDestination),
            6027 => Ok(Self::SpendingLimitExceeded),
            6028 => Ok(Self::DecimalsMismatch),
            6029 => Ok(Self::UnknownPermission),
            6030 => Ok(Self::ProtectedAccount),
            6031 => Ok(Self::TimeLockExceedsMaxAllowed),
            6032 => Ok(Self::IllegalAccountOwner),
            6033 => Ok(Self::RentReclamationDisabled),
            6034 => Ok(Self::InvalidRentCollector),
            6035 => Ok(Self::ProposalForAnotherSmartAccount),
            6036 => Ok(Self::TransactionForAnotherSmartAccount),
            6037 => Ok(Self::TransactionNotMatchingProposal),
            6038 => Ok(Self::TransactionNotLastInBatch),
            6039 => Ok(Self::BatchNotEmpty),
            6040 => Ok(Self::SpendingLimitInvalidAmount),
            6041 => Ok(Self::InvalidInstructionArgs),
            6042 => Ok(Self::FinalBufferHashMismatch),
            6043 => Ok(Self::FinalBufferSizeExceeded),
            6044 => Ok(Self::FinalBufferSizeMismatch),
            6045 => Ok(Self::SmartAccountCreateDeprecated),
            6046 => Ok(Self::ThresholdNotReached),
            6047 => Ok(Self::InvalidSignerCount),
            6048 => Ok(Self::MissingSignature),
            6049 => Ok(Self::InsufficientAggregatePermissions),
            6050 => Ok(Self::InsufficientVotePermissions),
            6051 => Ok(Self::TimeLockNotZero),
            6052 => Ok(Self::NotImplemented),
            6053 => Ok(Self::SpendingLimitInvalidCadenceConfiguration),
            6054 => Ok(Self::InvalidDataConstraint),
            6055 => Ok(Self::InvalidPayload),
            6056 => Ok(Self::ProtectedInstruction),
            6057 => Ok(Self::PlaceholderError),
            6058 => Ok(Self::InvalidPolicyPayload),
            6059 => Ok(Self::InvalidEmptyPolicy),
            6060 => Ok(Self::TransactionForAnotherPolicy),
            6061 => Ok(Self::ProgramInteractionAsyncPayloadNotAllowedWithSyncTransaction),
            6062 => Ok(Self::ProgramInteractionSyncPayloadNotAllowedWithAsyncTransaction),
            6063 => Ok(Self::ProgramInteractionDataTooShort),
            6064 => Ok(Self::ProgramInteractionInvalidNumericValue),
            6065 => Ok(Self::ProgramInteractionInvalidByteSequence),
            6066 => Ok(Self::ProgramInteractionUnsupportedSliceOperator),
            6067 => Ok(Self::ProgramInteractionDataParsingError),
            6068 => Ok(Self::ProgramInteractionProgramIdMismatch),
            6069 => Ok(Self::ProgramInteractionAccountConstraintViolated),
            6070 => Ok(Self::ProgramInteractionConstraintIndexOutOfBounds),
            6071 => Ok(Self::ProgramInteractionInstructionCountMismatch),
            6072 => Ok(Self::ProgramInteractionInsufficientLamportAllowance),
            6073 => Ok(Self::ProgramInteractionInsufficientTokenAllowance),
            6074 => Ok(Self::ProgramInteractionModifiedIllegalBalance),
            6075 => Ok(Self::ProgramInteractionIllegalTokenAccountModification),
            6076 => Ok(Self::ProgramInteractionDuplicateSpendingLimit),
            6077 => Ok(Self::ProgramInteractionTooManyInstructionConstraints),
            6078 => Ok(Self::ProgramInteractionTooManySpendingLimits),
            6079 => Ok(Self::ProgramInteractionTemplateHookError),
            6080 => Ok(Self::ProgramInteractionHookAuthorityCannotBePartOfHookAccounts),
            6081 => Ok(Self::SpendingLimitNotActive),
            6082 => Ok(Self::SpendingLimitExpired),
            6083 => Ok(Self::SpendingLimitPolicyInvariantAccumulateUnused),
            6084 => Ok(Self::SpendingLimitViolatesExactQuantityConstraint),
            6085 => Ok(Self::SpendingLimitViolatesMaxPerUseConstraint),
            6086 => Ok(Self::SpendingLimitInsufficientRemainingAmount),
            6087 => Ok(Self::SpendingLimitInvariantMaxPerPeriodZero),
            6088 => Ok(Self::SpendingLimitInvariantStartTimePositive),
            6089 => Ok(Self::SpendingLimitInvariantExpirationSmallerThanStart),
            6090 => Ok(Self::SpendingLimitInvariantOverflowEnabledMustHaveExpiration),
            6091 => Ok(Self::SpendingLimitInvariantOneTimePeriodCannotHaveOverflowEnabled),
            6092 => Ok(Self::SpendingLimitInvariantOverflowRemainingAmountGreaterThanMaxAmount),
            6093 => Ok(Self::SpendingLimitInvariantRemainingAmountGreaterThanMaxPerPeriod),
            6094 => Ok(Self::SpendingLimitInvariantExactQuantityMaxPerUseZero),
            6095 => Ok(Self::SpendingLimitInvariantMaxPerUseGreaterThanMaxPerPeriod),
            6096 => Ok(Self::SpendingLimitInvariantCustomPeriodNegative),
            6097 => Ok(Self::SpendingLimitPolicyInvariantDuplicateDestinations),
            6098 => Ok(Self::SpendingLimitInvariantLastResetOutOfBounds),
            6099 => Ok(Self::SpendingLimitInvariantLastResetSmallerThanStart),
            6100 => Ok(Self::InternalFundTransferPolicyInvariantSourceAccountIndexNotAllowed),
            6101 => Ok(Self::InternalFundTransferPolicyInvariantDestinationAccountIndexNotAllowed),
            6102 => Ok(Self::InternalFundTransferPolicyInvariantSourceAndDestinationCannotBeTheSame),
            6103 => Ok(Self::InternalFundTransferPolicyInvariantMintNotAllowed),
            6104 => Ok(Self::InternalFundTransferPolicyInvariantAmountZero),
            6105 => Ok(Self::InternalFundTransferPolicyInvariantDuplicateMints),
            6106 => Ok(Self::ConsensusAccountNotSettings),
            6107 => Ok(Self::ConsensusAccountNotPolicy),
            6108 => Ok(Self::SettingsChangePolicyActionsMustBeNonZero),
            6109 => Ok(Self::SettingsChangeInvalidSettingsKey),
            6110 => Ok(Self::SettingsChangeInvalidSettingsAccount),
            6111 => Ok(Self::SettingsChangeInvalidRentPayer),
            6112 => Ok(Self::SettingsChangeInvalidSystemProgram),
            6113 => Ok(Self::SettingsChangeAddSignerViolation),
            6114 => Ok(Self::SettingsChangeAddSignerPermissionsViolation),
            6115 => Ok(Self::SettingsChangeRemoveSignerViolation),
            6116 => Ok(Self::SettingsChangeChangeTimelockViolation),
            6117 => Ok(Self::SettingsChangeActionMismatch),
            6118 => Ok(Self::SettingsChangePolicyInvariantDuplicateActions),
            6119 => Ok(Self::SettingsChangePolicyInvariantActionIndicesActionsLengthMismatch),
            6120 => Ok(Self::SettingsChangePolicyInvariantActionIndexOutOfBounds),
            6121 => Ok(Self::PolicyNotActiveYet),
            6122 => Ok(Self::PolicyInvariantInvalidExpiration),
            6123 => Ok(Self::PolicyExpirationViolationPolicySettingsKeyMismatch),
            6124 => Ok(Self::PolicyExpirationViolationSettingsAccountNotPresent),
            6125 => Ok(Self::PolicyExpirationViolationHashExpired),
            6126 => Ok(Self::PolicyExpirationViolationTimestampExpired),
            _ => Err(()),
        }
    }
}
