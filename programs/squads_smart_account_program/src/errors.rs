use anchor_lang::prelude::*;

#[error_code]
pub enum SmartAccountError {
    #[msg("Account is not empty")]
    AccountNotEmpty,
    #[msg("Found multiple signers with the same pubkey")]
    DuplicateSigner,
    #[msg("Signers array is empty")]
    EmptySigners,
    #[msg("Too many signers, can be up to 65535")]
    TooManySigners,
    #[msg("Invalid threshold, must be between 1 and number of signers with vote permissions")]
    InvalidThreshold,
    #[msg("Attempted to perform an unauthorized action")]
    Unauthorized,
    #[msg("Provided pubkey is not a signer of the smart account")]
    NotASigner,
    #[msg("TransactionMessage is malformed.")]
    InvalidTransactionMessage,
    #[msg("Proposal is stale")]
    StaleProposal,
    #[msg("Invalid proposal status")]
    InvalidProposalStatus,
    #[msg("Invalid transaction index")]
    InvalidTransactionIndex,
    #[msg("Signer already approved the transaction")]
    AlreadyApproved,
    #[msg("Signer already rejected the transaction")]
    AlreadyRejected,
    #[msg("Signer already cancelled the transaction")]
    AlreadyCancelled,
    #[msg("Wrong number of accounts provided")]
    InvalidNumberOfAccounts,
    #[msg("Invalid account provided")]
    InvalidAccount,
    #[msg("Cannot remove last signer")]
    RemoveLastSigner,
    #[msg("Signers don't include any voters")]
    NoVoters,
    #[msg("Signers don't include any proposers")]
    NoProposers,
    #[msg("Signers don't include any executors")]
    NoExecutors,
    #[msg("`stale_transaction_index` must be <= `transaction_index`")]
    InvalidStaleTransactionIndex,
    #[msg("Instruction not supported for controlled smart account")]
    NotSupportedForControlled,
    #[msg("Proposal time lock has not been released")]
    TimeLockNotReleased,
    #[msg("Config transaction must have at least one action")]
    NoActions,
    #[msg("Missing account")]
    MissingAccount,
    #[msg("Invalid mint")]
    InvalidMint,
    #[msg("Invalid destination")]
    InvalidDestination,
    #[msg("Spending limit exceeded")]
    SpendingLimitExceeded,
    #[msg("Decimals don't match the mint")]
    DecimalsMismatch,
    #[msg("Signer has unknown permission")]
    UnknownPermission,
    #[msg("Account is protected, it cannot be passed into a CPI as writable")]
    ProtectedAccount,
    #[msg("Time lock exceeds the maximum allowed (90 days)")]
    TimeLockExceedsMaxAllowed,
    #[msg("Account is not owned by Smart Account program")]
    IllegalAccountOwner,
    #[msg("Rent reclamation is disabled for this smart account")]
    RentReclamationDisabled,
    #[msg("Invalid rent collector address")]
    InvalidRentCollector,
    #[msg("Proposal is for another smart account")]
    ProposalForAnotherSmartAccount,
    #[msg("Transaction is for another smart account")]
    TransactionForAnotherSmartAccount,
    #[msg("Transaction doesn't match proposal")]
    TransactionNotMatchingProposal,
    #[msg("Transaction is not last in batch")]
    TransactionNotLastInBatch,
    #[msg("Batch is not empty")]
    BatchNotEmpty,
    #[msg("Invalid SpendingLimit amount")]
    SpendingLimitInvalidAmount,
    #[msg("Invalid Instruction Arguments")]
    InvalidInstructionArgs,
    #[msg("Final message buffer hash doesnt match the expected hash")]
    FinalBufferHashMismatch,
    #[msg("Final buffer size cannot exceed 4000 bytes")]
    FinalBufferSizeExceeded,
    #[msg("Final buffer size mismatch")]
    FinalBufferSizeMismatch,
    #[msg("smart_account_create has been deprecated. Use smart_account_create_v2 instead.")]
    SmartAccountCreateDeprecated,
    #[msg("Signers do not reach consensus threshold")]
    ThresholdNotReached,
    #[msg("Invalid number of signer accounts. Must be greater or equal to the threshold")]
    InvalidSignerCount,
    #[msg("Missing signature")]
    MissingSignature,
    #[msg("Insufficient aggregate permissions across signing members")]
    InsufficientAggregatePermissions,
    #[msg("Insufficient vote permissions across signing members")]
    InsufficientVotePermissions,
    #[msg("Smart account must not be time locked")]
    TimeLockNotZero,
    #[msg("Feature not implemented")]
    NotImplemented,
    #[msg("Invalid cadence configuration")]
    SpendingLimitInvalidCadenceConfiguration,
    #[msg("Invalid data constraint")]
    InvalidDataConstraint,


    #[msg("Invalid payload")]
    InvalidPayload,
    #[msg("Protected instruction")]
    ProtectedInstruction,
    #[msg("Placeholder error")]
    PlaceholderError,

    // ===============================================
    // Overall Policy Errors
    // ===============================================
    #[msg("Invalid policy payload")]
    InvalidPolicyPayload,
    #[msg("Invalid empty policy")]
    InvalidEmptyPolicy,
    #[msg("Transaction is for another policy")]
    TransactionForAnotherPolicy,

    // ===============================================
    // Program Interaction Policy Errors
    // ===============================================
    #[msg("Program interaction sync payload not allowed with async transaction")]
    ProgramInteractionAsyncPayloadNotAllowedWithSyncTransaction,
    #[msg("Program interaction sync payload not allowed with sync transaction")]
    ProgramInteractionSyncPayloadNotAllowedWithAsyncTransaction,
    #[msg("Program interaction data constraint failed: instruction data too short")]
    ProgramInteractionDataTooShort,
    #[msg("Program interaction data constraint failed: invalid numeric value")]
    ProgramInteractionInvalidNumericValue,
    #[msg("Program interaction data constraint failed: invalid byte sequence")]
    ProgramInteractionInvalidByteSequence,
    #[msg("Program interaction data constraint failed: unsupported operator for byte slice")]
    ProgramInteractionUnsupportedSliceOperator,
    #[msg("Program interaction constraint failed: instruction data parsing error")]
    ProgramInteractionDataParsingError,
    #[msg("Program interaction constraint failed: program ID mismatch")]
    ProgramInteractionProgramIdMismatch,
    #[msg("Program interaction constraint violation: account constraint")]
    ProgramInteractionAccountConstraintViolated,
    #[msg("Program interaction constraint violation: instruction constraint index out of bounds")]
    ProgramInteractionConstraintIndexOutOfBounds,
    #[msg("Program interaction constraint violation: instruction count mismatch")]
    ProgramInteractionInstructionCountMismatch,
    #[msg("Program interaction constraint violation: insufficient remaining lamport allowance")]
    ProgramInteractionInsufficientLamportAllowance,
    #[msg("Program interaction constraint violation: insufficient remaining token allowance")]
    ProgramInteractionInsufficientTokenAllowance,
    #[msg("Program interaction constraint violation: modified illegal balance")]
    ProgramInteractionModifiedIllegalBalance,
    #[msg("Program interaction constraint violation: illegal token account modification")]
    ProgramInteractionIllegalTokenAccountModification,
    #[msg("Program interaction invariant violation: duplicate spending limit for the same mint")]
    ProgramInteractionDuplicateSpendingLimit,

    // ===============================================
    // Spending Limit Policy Errors
    // ===============================================
    #[msg("Spending limit is not active")]
    SpendingLimitNotActive,
    #[msg("Spending limit is expired")]
    SpendingLimitExpired,
    #[msg("Spending limit policy invariant violation: usage state cannot be Some() if accumulate_unused is true")]
    SpendingLimitPolicyInvariantAccumulateUnused,
    #[msg("Amount violates exact quantity constraint")]
    SpendingLimitViolatesExactQuantityConstraint,
    #[msg("Amount violates max per use constraint")]
    SpendingLimitViolatesMaxPerUseConstraint,
    #[msg("Spending limit is insufficient")]
    SpendingLimitInsufficientRemainingAmount,
    #[msg("Spending limit invariant violation: max per period must be non-zero")]
    SpendingLimitInvariantMaxPerPeriodZero,
    #[msg("Spending limit invariant violation: start time must be positive")]
    SpendingLimitInvariantStartTimePositive,
    #[msg("Spending limit invariant violation: expiration must be greater than start")]
    SpendingLimitInvariantExpirationSmallerThanStart,
    #[msg("Spending limit invariant violation: overflow enabled must have expiration")]
    SpendingLimitInvariantOverflowEnabledMustHaveExpiration,
    #[msg("Spending limit invariant violation: one time period cannot have overflow enabled")]
    SpendingLimitInvariantOneTimePeriodCannotHaveOverflowEnabled,
    #[msg("Spending limit invariant violation: remaining amount must be less than max amount")]
    SpendingLimitInvariantOverflowRemainingAmountGreaterThanMaxAmount,
    #[msg("Spending limit invariant violation: remaining amount must be less than or equal to max per period")]
    SpendingLimitInvariantRemainingAmountGreaterThanMaxPerPeriod,
    #[msg("Spending limit invariant violation: exact quantity must have max per use non-zero")]
    SpendingLimitInvariantExactQuantityMaxPerUseZero,
    #[msg("Spending limit invariant violation: max per use must be less than or equal to max per period")]
    SpendingLimitInvariantMaxPerUseGreaterThanMaxPerPeriod,
    #[msg("Spending limit invariant violation: custom period must be positive")]
    SpendingLimitInvariantCustomPeriodNegative,
    #[msg("Spending limit policy invariant violation: cannot have duplicate destinations for the same mint")]
    SpendingLimitPolicyInvariantDuplicateDestinations,

    // ===============================================
    // Internal Fund Transfer Policy Errors
    // ===============================================
    #[msg("Internal fund transfer policy invariant violation: source account index is not allowed")]
    InternalFundTransferPolicyInvariantSourceAccountIndexNotAllowed,
    #[msg("Internal fund transfer policy invariant violation: destination account index is not allowed")]
    InternalFundTransferPolicyInvariantDestinationAccountIndexNotAllowed,
    #[msg("Internal fund transfer policy invariant violation: source and destination cannot be the same")]
    InternalFundTransferPolicyInvariantSourceAndDestinationCannotBeTheSame,
    #[msg("Internal fund transfer policy invariant violation: mint is not allowed")]
    InternalFundTransferPolicyInvariantMintNotAllowed,
    #[msg("Internal fund transfer policy invariant violation: amount must be greater than 0")]
    InternalFundTransferPolicyInvariantAmountZero,
    #[msg("Internal fund transfer policy invariant violation: cannot have duplicate mints")]
    InternalFundTransferPolicyInvariantDuplicateMints,

    // ===============================================
    // Consensus Account Errors
    // ===============================================
    #[msg("Consensus account is not a settings")]
    ConsensusAccountNotSettings,
    #[msg("Consensus account is not a policy")]
    ConsensusAccountNotPolicy,

    // ===============================================
    // Settings Change Policy Errors
    // ===============================================
    #[msg("Settings change policy invariant violation: actions must be non-zero")]
    SettingsChangePolicyActionsMustBeNonZero,
    #[msg("Settings change policy violation: submitted settings account must match policy settings key")]
    SettingsChangeInvalidSettingsKey,
    #[msg("Settings change policy violation: submitted settings account must be writable")]
    SettingsChangeInvalidSettingsAccount,
    #[msg("Settings change policy violation: rent payer must be writable and signer")]
    SettingsChangeInvalidRentPayer,
    #[msg("Settings change policy violation: system program must be the system program")]
    SettingsChangeInvalidSystemProgram,
    #[msg("Settings change policy violation: signer does not match allowed signer")]
    SettingsChangeAddSignerViolation,
    #[msg("Settings change policy violation: signer permissions does not match allowed signer permissions")]
    SettingsChangeAddSignerPermissionsViolation,
    #[msg("Settings change policy violation: signer removal does not mach allowed signer removal")]
    SettingsChangeRemoveSignerViolation,
    #[msg("Settings change policy violation: time lock does not match allowed time lock")]
    SettingsChangeChangeTimelockViolation,
    #[msg("Settings change policy violation: action does not match allowed action")]
    SettingsChangeActionMismatch,
    #[msg("Settings change policy invariant violation: cannot have duplicate actions")]
    SettingsChangePolicyInvariantDuplicateActions,
    #[msg("Settings change policy invariant violation: action indices must match actions length")]
    SettingsChangePolicyInvariantActionIndicesActionsLengthMismatch,
    #[msg("Settings change policy invariant violation: action index out of bounds")]
    SettingsChangePolicyInvariantActionIndexOutOfBounds,

    // ===============================================
    // Policy Expiration Errors
    // ===============================================
    #[msg("Policy is not active yet")]
    PolicyNotActiveYet,
    #[msg("Policy expiration violation: submitted settings key does not match policy settings key")]
    PolicyExpirationViolationPolicySettingsKeyMismatch,
    #[msg("Policy expiration violation: state expiration requires the settings to be submitted")]
    PolicyExpirationViolationSettingsAccountNotPresent,
    #[msg("Policy expiration violation: state hash has expired")]
    PolicyExpirationViolationHashExpired,
    #[msg("Policy expiration violation: timestamp has expired")]
    PolicyExpirationViolationTimestampExpired,
}
