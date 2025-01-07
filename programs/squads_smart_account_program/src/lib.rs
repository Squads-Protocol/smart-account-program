#![allow(clippy::result_large_err)]
#![deny(arithmetic_overflow)]
#![deny(unused_must_use)]
// #![deny(clippy::arithmetic_side_effects)]
// #![deny(clippy::integer_arithmetic)]

// Re-export anchor_lang for convenience.
pub use anchor_lang;
use anchor_lang::prelude::*;
#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

pub use instructions::ProgramConfig;
pub use instructions::*;
pub use state::*;
pub use utils::SmallVec;

pub mod allocator;
pub mod errors;
pub mod instructions;
pub mod state;
mod utils;

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Squads Smart Account Program",
    project_url: "https://squads.so",
    contacts: "email:security@sqds.io,email:contact@osec.io",
    policy: "https://github.com/Squads-Protocol/v4/blob/main/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/squads-protocol/v4",
    auditors: "OtterSec, Neodyme"
}

#[cfg(not(feature = "testing"))]
declare_id!("SMRTjqhGQ29cvyp44FWPTMPdQ722qCcWWcU2fWzgjzm");

#[cfg(feature = "testing")]
declare_id!("GyhGAqjokLwF9UXdQ2dR5Zwiup242j4mX4J1tSMKyAmD");

#[program]
pub mod squads_smart_account_program {

    use super::*;

    /// Initialize the program config.
    pub fn initialize_program_config(
        ctx: Context<InitProgramConfig>,
        args: InitProgramConfigArgs,
    ) -> Result<()> {
        InitProgramConfig::init_program_config(ctx, args)
    }

    /// Set the `authority` parameter of the program config.
    pub fn set_program_config_authority(
        ctx: Context<ProgramConfig>,
        args: ProgramConfigSetAuthorityArgs,
    ) -> Result<()> {
        ProgramConfig::set_authority(ctx, args)
    }

    /// Set the `multisig_creation_fee` parameter of the program config.
    pub fn set_program_config_smart_account_creation_fee(
        ctx: Context<ProgramConfig>,
        args: ProgramConfigSetSmartAccountCreationFeeArgs,
    ) -> Result<()> {
        ProgramConfig::set_smart_account_creation_fee(ctx, args)
    }

    /// Set the `treasury` parameter of the program config.
    pub fn set_program_config_treasury(
        ctx: Context<ProgramConfig>,
        args: ProgramConfigSetTreasuryArgs,
    ) -> Result<()> {
        ProgramConfig::set_treasury(ctx, args)
    }
    /// Create a smart account.
    pub fn create_smart_account(
        ctx: Context<CreateSmartAccount>,
        args: CreateSmartAccountArgs,
    ) -> Result<()> {
        CreateSmartAccount::create_smart_account(ctx, args)
    }

    /// Add a new signer to the controlled multisig.
    pub fn add_signer_as_authority(
        ctx: Context<ExecuteSettingsTransactionAsAuthority>,
        args: AddSignerArgs,
    ) -> Result<()> {
        ExecuteSettingsTransactionAsAuthority::add_signer(ctx, args)
    }

    /// Remove a signer from the controlled multisig.
    pub fn remove_signer_as_authority(
        ctx: Context<ExecuteSettingsTransactionAsAuthority>,
        args: RemoveSignerArgs,
    ) -> Result<()> {
        ExecuteSettingsTransactionAsAuthority::remove_signer(ctx, args)
    }

    /// Set the `time_lock` config parameter for the controlled multisig.
    pub fn set_time_lock_as_authority(
        ctx: Context<ExecuteSettingsTransactionAsAuthority>,
        args: SetTimeLockArgs,
    ) -> Result<()> {
        ExecuteSettingsTransactionAsAuthority::set_time_lock(ctx, args)
    }

    /// Set the `threshold` config parameter for the controlled multisig.
    pub fn change_threshold_as_authority(
        ctx: Context<ExecuteSettingsTransactionAsAuthority>,
        args: ChangeThresholdArgs,
    ) -> Result<()> {
        ExecuteSettingsTransactionAsAuthority::change_threshold(ctx, args)
    }

    /// Set the multisig `config_authority`.
    pub fn set_new_settings_authority_as_authority(
        ctx: Context<ExecuteSettingsTransactionAsAuthority>,
        args: SetNewSettingsAuthorityArgs,
    ) -> Result<()> {
        ExecuteSettingsTransactionAsAuthority::set_new_settings_authority(ctx, args)
    }

    /// Set the multisig `archival_authority`.
    pub fn set_archival_authority_as_authority(
        ctx: Context<ExecuteSettingsTransactionAsAuthority>,
        args: SetArchivalAuthorityArgs,
    ) -> Result<()> {
        ExecuteSettingsTransactionAsAuthority::set_archival_authority(ctx, args)
    }

    /// Create a new spending limit for the controlled multisig.
    pub fn add_spending_limit_as_authority(
        ctx: Context<AddSpendingLimitAsAuthority>,
        args: AddSpendingLimitArgs,
    ) -> Result<()> {
        AddSpendingLimitAsAuthority::add_spending_limit(ctx, args)
    }

    /// Remove the spending limit from the controlled multisig.
    pub fn remove_spending_limit_as_authority(
        ctx: Context<RemoveSpendingLimitAsAuthority>,
        args: RemoveSpendingLimitArgs,
    ) -> Result<()> {
        RemoveSpendingLimitAsAuthority::remove_spending_limit(ctx, args)
    }

    /// Create a new settings transaction.
    pub fn create_settings_transaction(
        ctx: Context<CreateSettingsTransaction>,
        args: CreateSettingsTransactionArgs,
    ) -> Result<()> {
        CreateSettingsTransaction::create_settings_transaction(ctx, args)
    }

    /// Execute a settings transaction.
    /// The transaction must be `Approved`.
    pub fn execute_settings_transaction<'info>(
        ctx: Context<'_, '_, 'info, 'info, ExecuteSettingsTransaction<'info>>,
    ) -> Result<()> {
        ExecuteSettingsTransaction::execute_settings_transaction(ctx)
    }

    /// Create a new vault transaction.
    pub fn create_transaction(
        ctx: Context<CreateTransaction>,
        args: CreateTransactionArgs,
    ) -> Result<()> {
        CreateTransaction::create_transaction(ctx, args)
    }

    /// Create a transaction buffer account.
    pub fn create_transaction_buffer(
        ctx: Context<CreateTransactionBuffer>,
        args: CreateTransactionBufferArgs,
    ) -> Result<()> {
        CreateTransactionBuffer::create_transaction_buffer(ctx, args)
    }

    /// Close a transaction buffer account.
    pub fn close_transaction_buffer(ctx: Context<CloseTransactionBuffer>) -> Result<()> {
        CloseTransactionBuffer::close_transaction_buffer(ctx)
    }

    /// Extend a transaction buffer account.
    pub fn extend_transaction_buffer(
        ctx: Context<ExtendTransactionBuffer>,
        args: ExtendTransactionBufferArgs,
    ) -> Result<()> {
        ExtendTransactionBuffer::extend_transaction_buffer(ctx, args)
    }

    /// Create a new vault transaction from a completed transaction buffer.
    /// Finalized buffer hash must match `final_buffer_hash`
    pub fn create_transaction_from_buffer<'info>(
        ctx: Context<'_, '_, 'info, 'info, CreateTransactionFromBuffer<'info>>,
        args: CreateTransactionArgs,
    ) -> Result<()> {
        CreateTransactionFromBuffer::create_transaction_from_buffer(ctx, args)
    }

    /// Execute a smart account transaction.
    /// The transaction must be `Approved`.
    pub fn execute_transaction(ctx: Context<ExecuteTransaction>) -> Result<()> {
        ExecuteTransaction::execute_transaction(ctx)
    }

    /// Create a new batch.
    pub fn create_batch(ctx: Context<CreateBatch>, args: CreateBatchArgs) -> Result<()> {
        CreateBatch::create_batch(ctx, args)
    }

    /// Add a transaction to the batch.
    pub fn add_transaction_to_batch(
        ctx: Context<AddTransactionToBatch>,
        args: AddTransactionToBatchArgs,
    ) -> Result<()> {
        AddTransactionToBatch::add_transaction_to_batch(ctx, args)
    }

    /// Execute a transaction from the batch.
    pub fn execute_batch_transaction(ctx: Context<ExecuteBatchTransaction>) -> Result<()> {
        ExecuteBatchTransaction::execute_batch_transaction(ctx)
    }

    /// Create a new multisig proposal.
    pub fn create_proposal(ctx: Context<CreateProposal>, args: CreateProposalArgs) -> Result<()> {
        CreateProposal::create_proposal(ctx, args)
    }

    /// Update status of a multisig proposal from `Draft` to `Active`.
    pub fn activate_proposal(ctx: Context<ActivateProposal>) -> Result<()> {
        ActivateProposal::activate_proposal(ctx)
    }

    /// Approve a multisig proposal on behalf of the `member`.
    /// The proposal must be `Active`.
    pub fn approve_proposal(ctx: Context<VoteOnProposal>, args: VoteOnProposalArgs) -> Result<()> {
        VoteOnProposal::approve_proposal(ctx, args)
    }

    /// Reject a multisig proposal on behalf of the `member`.
    /// The proposal must be `Active`.
    pub fn reject_proposal(ctx: Context<VoteOnProposal>, args: VoteOnProposalArgs) -> Result<()> {
        VoteOnProposal::reject_proposal(ctx, args)
    }

    /// Cancel a multisig proposal on behalf of the `member`.
    /// The proposal must be `Approved`.
    pub fn cancel_proposal(ctx: Context<VoteOnProposal>, args: VoteOnProposalArgs) -> Result<()> {
        VoteOnProposal::cancel_proposal(ctx, args)
    }

    /// Use a spending limit to transfer tokens from a multisig vault to a destination account.
    pub fn use_spending_limit(
        ctx: Context<UseSpendingLimit>,
        args: UseSpendingLimitArgs,
    ) -> Result<()> {
        UseSpendingLimit::use_spending_limit(ctx, args)
    }

    /// Closes a `SettingsTransaction` and the corresponding `Proposal`.
    /// `transaction` can be closed if either:
    /// - the `proposal` is in a terminal state: `Executed`, `Rejected`, or `Cancelled`.
    /// - the `proposal` is stale.
    pub fn close_settings_transaction(ctx: Context<CloseSettingsTransaction>) -> Result<()> {
        CloseSettingsTransaction::close_settings_transaction(ctx)
    }

    /// Closes a `Transaction` and the corresponding `Proposal`.
    /// `transaction` can be closed if either:
    /// - the `proposal` is in a terminal state: `Executed`, `Rejected`, or `Cancelled`.
    /// - the `proposal` is stale and not `Approved`.
    pub fn close_transaction(ctx: Context<CloseTransaction>) -> Result<()> {
        CloseTransaction::close_transaction(ctx)
    }

    /// Closes a `BatchTransaction` belonging to the `batch` and `proposal`.
    /// `transaction` can be closed if either:
    /// - it's marked as executed within the `batch`;
    /// - the `proposal` is in a terminal state: `Executed`, `Rejected`, or `Cancelled`.
    /// - the `proposal` is stale and not `Approved`.
    pub fn close_batch_transaction(ctx: Context<CloseBatchTransaction>) -> Result<()> {
        CloseBatchTransaction::close_batch_transaction(ctx)
    }

    /// Closes Batch and the corresponding Proposal accounts for proposals in terminal states:
    /// `Executed`, `Rejected`, or `Cancelled` or stale proposals that aren't `Approved`.
    ///
    /// This instruction is only allowed to be executed when all `VaultBatchTransaction` accounts
    /// in the `batch` are already closed: `batch.size == 0`.
    pub fn close_batch(ctx: Context<CloseBatch>) -> Result<()> {
        CloseBatch::close_batch(ctx)
    }

    /// Synchronously execute a transaction
    pub fn execute_transaction_sync(
        ctx: Context<SyncTransaction>,
        args: SyncTransactionArgs,
    ) -> Result<()> {
        SyncTransaction::sync_transaction(ctx, args)
    }

    /// Synchronously execute a config transaction
    pub fn execute_settings_transaction_sync<'info>(
        ctx: Context<'_, '_, 'info, 'info, SyncSettingsTransaction<'info>>,
        args: SyncSettingsTransactionArgs,
    ) -> Result<()> {
        SyncSettingsTransaction::sync_settings_transaction(ctx, args)
    }
}
