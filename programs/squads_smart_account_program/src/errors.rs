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
    #[msg("Spending limit is expired")]
    SpendingLimitExpired,
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
    #[msg("Invalid policy payload")]
    InvalidPolicyPayload,
    #[msg("Policy expired")]
    PolicyExpired,
    #[msg("Consensus account is not a policy")]
    ConsensusAccountNotPolicy,
    #[msg("Invalid payload")]
    InvalidPayload,
    #[msg("Protected instruction")]
    ProtectedInstruction,
    #[msg("Placeholder error")]
    PlaceholderError,

    // ===============================================
    // Program Interaction Policy Errors
    // ===============================================
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
    #[msg("Program interaction invariant violation: duplicate resource limit for the same mint")]
    ProgramInteractionDuplicateResourceLimit,
}
