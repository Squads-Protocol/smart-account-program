use anchor_lang::prelude::*;

#[error_code]
pub enum SmartAccountError {
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
    #[msg("Instruction not supported for controlled multisig")]
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
}
