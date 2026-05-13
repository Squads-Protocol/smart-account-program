//! Off-chain instruction builders for the Squads Smart Account program.
//!
//! Each builder takes the caller-controlled account pubkeys plus the
//! corresponding args struct, computes the Anchor `global:<name>` discriminator,
//! borsh-serializes the args, and assembles the `Instruction` with the correct
//! account order and signer/writable flags expected by the on-chain program.
//!
//! Built only when the `instructions` feature is enabled. Uses the in-crate
//! `Instruction` and `AccountMeta` types defined in `ix_types`, which mirror
//! `solana_instruction` field-for-field. This keeps the feature free of any
//! `solana-program` / `solana-instruction` dependency — only `solana-pubkey`,
//! `borsh`, and `sha2` are required.

use borsh::BorshSerialize;
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

use crate::instructions::ix_types::{AccountMeta, Instruction};
use crate::{instructions::*, PROGRAM_ID};

/// Solana System program ID (`11111111111111111111111111111111` is the
/// base58 encoding of the all-zero 32-byte array).
const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0u8; 32]);

/// Compute the Anchor 8-byte discriminator for an instruction by name.
/// Matches Anchor's codegen: `sha256("global:<snake_case_name>")[..8]`.
fn ix_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(b"global:");
    hasher.update(name.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

/// Borsh-serialize args, prepended with the Anchor discriminator.
fn encode_ix_data<A: BorshSerialize>(name: &str, args: &A) -> Vec<u8> {
    let mut data = ix_discriminator(name).to_vec();
    args.serialize(&mut data).expect("borsh serialize failed");
    data
}

/// Encode an instruction with no args — just the discriminator.
fn encode_empty_ix_data(name: &str) -> Vec<u8> {
    ix_discriminator(name).to_vec()
}

// ============================================================================
// initialize_program_config
// ============================================================================

pub struct InitializeProgramConfigAccounts {
    pub program_config: Pubkey,
    pub initializer: Pubkey,
}

pub fn initialize_program_config_ix(
    accounts: InitializeProgramConfigAccounts,
    args: InitProgramConfigArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(accounts.program_config, false),
            AccountMeta::new(accounts.initializer, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: encode_ix_data("initialize_program_config", &args),
    }
}

// ============================================================================
// create_smart_account
// ============================================================================

pub struct CreateSmartAccountAccounts {
    pub program_config: Pubkey,
    pub treasury: Pubkey,
    pub creator: Pubkey,
}

pub fn create_smart_account_ix(
    accounts: CreateSmartAccountAccounts,
    args: CreateSmartAccountArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(accounts.program_config, false),
            AccountMeta::new(accounts.treasury, false),
            AccountMeta::new(accounts.creator, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ],
        data: encode_ix_data("create_smart_account", &args),
    }
}

// ============================================================================
// Program config setters (share `ProgramConfig` accounts struct)
// ============================================================================

/// Accounts for the `set_program_config_*` instructions.
pub struct ProgramConfigAccounts {
    pub program_config: Pubkey,
    pub authority: Pubkey,
}

fn program_config_metas(accounts: &ProgramConfigAccounts) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(accounts.program_config, false),
        AccountMeta::new_readonly(accounts.authority, true),
    ]
}

pub fn set_program_config_authority_ix(
    accounts: ProgramConfigAccounts,
    args: ProgramConfigSetAuthorityArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: program_config_metas(&accounts),
        data: encode_ix_data("set_program_config_authority", &args),
    }
}

pub fn set_program_config_smart_account_creation_fee_ix(
    accounts: ProgramConfigAccounts,
    args: ProgramConfigSetSmartAccountCreationFeeArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: program_config_metas(&accounts),
        data: encode_ix_data("set_program_config_smart_account_creation_fee", &args),
    }
}

pub fn set_program_config_treasury_ix(
    accounts: ProgramConfigAccounts,
    args: ProgramConfigSetTreasuryArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: program_config_metas(&accounts),
        data: encode_ix_data("set_program_config_treasury", &args),
    }
}

// ============================================================================
// Authority-controlled settings transaction execute (share accounts struct)
// ============================================================================

/// Accounts for the `*_as_authority` settings instructions.
///
/// `rent_payer` and `system_program` are optional. When omitted, the program
/// sentinel pubkey is sent in their slot per Anchor's `allow-missing-optionals`
/// convention.
pub struct ExecuteSettingsTransactionAsAuthorityAccounts {
    pub settings: Pubkey,
    pub settings_authority: Pubkey,
    pub rent_payer: Option<Pubkey>,
}

fn execute_settings_transaction_as_authority_metas(
    accounts: &ExecuteSettingsTransactionAsAuthorityAccounts,
) -> Vec<AccountMeta> {
    let rent_payer_meta = match accounts.rent_payer {
        Some(pk) => AccountMeta::new(pk, true),
        None => AccountMeta::new_readonly(PROGRAM_ID, false),
    };
    // system_program is optional: present iff rent_payer is present.
    let system_program_meta = match accounts.rent_payer {
        Some(_) => AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        None => AccountMeta::new_readonly(PROGRAM_ID, false),
    };
    vec![
        AccountMeta::new(accounts.settings, false),
        AccountMeta::new_readonly(accounts.settings_authority, true),
        rent_payer_meta,
        system_program_meta,
        AccountMeta::new_readonly(PROGRAM_ID, false),
    ]
}

pub fn add_signer_as_authority_ix(
    accounts: ExecuteSettingsTransactionAsAuthorityAccounts,
    args: AddSignerArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: execute_settings_transaction_as_authority_metas(&accounts),
        data: encode_ix_data("add_signer_as_authority", &args),
    }
}

pub fn remove_signer_as_authority_ix(
    accounts: ExecuteSettingsTransactionAsAuthorityAccounts,
    args: RemoveSignerArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: execute_settings_transaction_as_authority_metas(&accounts),
        data: encode_ix_data("remove_signer_as_authority", &args),
    }
}

pub fn set_time_lock_as_authority_ix(
    accounts: ExecuteSettingsTransactionAsAuthorityAccounts,
    args: SetTimeLockArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: execute_settings_transaction_as_authority_metas(&accounts),
        data: encode_ix_data("set_time_lock_as_authority", &args),
    }
}

pub fn change_threshold_as_authority_ix(
    accounts: ExecuteSettingsTransactionAsAuthorityAccounts,
    args: ChangeThresholdArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: execute_settings_transaction_as_authority_metas(&accounts),
        data: encode_ix_data("change_threshold_as_authority", &args),
    }
}

pub fn set_new_settings_authority_as_authority_ix(
    accounts: ExecuteSettingsTransactionAsAuthorityAccounts,
    args: SetNewSettingsAuthorityArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: execute_settings_transaction_as_authority_metas(&accounts),
        data: encode_ix_data("set_new_settings_authority_as_authority", &args),
    }
}

pub fn set_archival_authority_as_authority_ix(
    accounts: ExecuteSettingsTransactionAsAuthorityAccounts,
    args: SetArchivalAuthorityArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: execute_settings_transaction_as_authority_metas(&accounts),
        data: encode_ix_data("set_archival_authority_as_authority", &args),
    }
}

// ============================================================================
// add_spending_limit_as_authority
// ============================================================================

pub struct AddSpendingLimitAsAuthorityAccounts {
    pub settings: Pubkey,
    pub settings_authority: Pubkey,
    pub spending_limit: Pubkey,
    pub rent_payer: Pubkey,
}

pub fn add_spending_limit_as_authority_ix(
    accounts: AddSpendingLimitAsAuthorityAccounts,
    args: AddSpendingLimitArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.settings, false),
            AccountMeta::new_readonly(accounts.settings_authority, true),
            AccountMeta::new(accounts.spending_limit, false),
            AccountMeta::new(accounts.rent_payer, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ],
        data: encode_ix_data("add_spending_limit_as_authority", &args),
    }
}

// ============================================================================
// remove_spending_limit_as_authority
// ============================================================================

pub struct RemoveSpendingLimitAsAuthorityAccounts {
    pub settings: Pubkey,
    pub settings_authority: Pubkey,
    pub spending_limit: Pubkey,
    pub rent_collector: Pubkey,
}

pub fn remove_spending_limit_as_authority_ix(
    accounts: RemoveSpendingLimitAsAuthorityAccounts,
    args: RemoveSpendingLimitArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.settings, false),
            AccountMeta::new_readonly(accounts.settings_authority, true),
            AccountMeta::new(accounts.spending_limit, false),
            AccountMeta::new(accounts.rent_collector, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ],
        data: encode_ix_data("remove_spending_limit_as_authority", &args),
    }
}

// ============================================================================
// create_settings_transaction
// ============================================================================

pub struct CreateSettingsTransactionAccounts {
    pub settings: Pubkey,
    pub transaction: Pubkey,
    pub creator: Pubkey,
    pub rent_payer: Pubkey,
}

pub fn create_settings_transaction_ix(
    accounts: CreateSettingsTransactionAccounts,
    args: CreateSettingsTransactionArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(accounts.settings, false),
            AccountMeta::new(accounts.transaction, false),
            AccountMeta::new_readonly(accounts.creator, true),
            AccountMeta::new(accounts.rent_payer, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ],
        data: encode_ix_data("create_settings_transaction", &args),
    }
}

// ============================================================================
// execute_settings_transaction
// ============================================================================

/// Accounts for `execute_settings_transaction`.
///
/// Reads `ctx.remaining_accounts` for any SpendingLimit accounts that need to
/// be initialized/closed in response to the transaction's actions.
pub struct ExecuteSettingsTransactionAccounts {
    pub settings: Pubkey,
    pub signer: Pubkey,
    pub proposal: Pubkey,
    pub transaction: Pubkey,
    pub rent_payer: Option<Pubkey>,
    pub additional_accounts: Vec<AccountMeta>,
}

pub fn execute_settings_transaction_ix(
    accounts: ExecuteSettingsTransactionAccounts,
) -> Instruction {
    let rent_payer_meta = match accounts.rent_payer {
        Some(pk) => AccountMeta::new(pk, true),
        None => AccountMeta::new_readonly(PROGRAM_ID, false),
    };
    let system_program_meta = match accounts.rent_payer {
        Some(_) => AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        None => AccountMeta::new_readonly(PROGRAM_ID, false),
    };
    let mut metas = vec![
        AccountMeta::new(accounts.settings, false),
        AccountMeta::new_readonly(accounts.signer, true),
        AccountMeta::new(accounts.proposal, false),
        AccountMeta::new_readonly(accounts.transaction, false),
        rent_payer_meta,
        system_program_meta,
        AccountMeta::new_readonly(PROGRAM_ID, false),
    ];
    metas.extend(accounts.additional_accounts);
    Instruction {
        program_id: PROGRAM_ID,
        accounts: metas,
        data: encode_empty_ix_data("execute_settings_transaction"),
    }
}

// ============================================================================
// create_transaction
// ============================================================================

pub struct CreateTransactionAccounts {
    pub consensus_account: Pubkey,
    pub transaction: Pubkey,
    pub creator: Pubkey,
    pub rent_payer: Pubkey,
}

pub fn create_transaction_ix(
    accounts: CreateTransactionAccounts,
    args: CreateTransactionArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(accounts.consensus_account, false),
            AccountMeta::new(accounts.transaction, false),
            AccountMeta::new_readonly(accounts.creator, true),
            AccountMeta::new(accounts.rent_payer, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ],
        data: encode_ix_data("create_transaction", &args),
    }
}

// ============================================================================
// create_transaction_buffer
// ============================================================================

pub struct CreateTransactionBufferAccounts {
    pub consensus_account: Pubkey,
    pub transaction_buffer: Pubkey,
    pub creator: Pubkey,
    pub rent_payer: Pubkey,
}

pub fn create_transaction_buffer_ix(
    accounts: CreateTransactionBufferAccounts,
    args: CreateTransactionBufferArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.consensus_account, false),
            AccountMeta::new(accounts.transaction_buffer, false),
            AccountMeta::new_readonly(accounts.creator, true),
            AccountMeta::new(accounts.rent_payer, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: encode_ix_data("create_transaction_buffer", &args),
    }
}

// ============================================================================
// close_transaction_buffer
// ============================================================================

pub struct CloseTransactionBufferAccounts {
    pub consensus_account: Pubkey,
    pub transaction_buffer: Pubkey,
    pub creator: Pubkey,
}

pub fn close_transaction_buffer_ix(accounts: CloseTransactionBufferAccounts) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.consensus_account, false),
            AccountMeta::new(accounts.transaction_buffer, false),
            AccountMeta::new_readonly(accounts.creator, true),
        ],
        data: encode_empty_ix_data("close_transaction_buffer"),
    }
}

// ============================================================================
// extend_transaction_buffer
// ============================================================================

pub struct ExtendTransactionBufferAccounts {
    pub consensus_account: Pubkey,
    pub transaction_buffer: Pubkey,
    pub creator: Pubkey,
}

pub fn extend_transaction_buffer_ix(
    accounts: ExtendTransactionBufferAccounts,
    args: ExtendTransactionBufferArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.consensus_account, false),
            AccountMeta::new(accounts.transaction_buffer, false),
            AccountMeta::new_readonly(accounts.creator, true),
        ],
        data: encode_ix_data("extend_transaction_buffer", &args),
    }
}

// ============================================================================
// create_transaction_from_buffer
// ============================================================================

/// Accounts for `create_transaction_from_buffer`.
///
/// Composes the inner `CreateTransaction` accounts plus the buffer & creator.
/// The handler reads `ctx.remaining_accounts` (forwarded to the inner
/// `create_transaction` call).
pub struct CreateTransactionFromBufferAccounts {
    // Inner CreateTransaction accounts
    pub consensus_account: Pubkey,
    pub transaction: Pubkey,
    pub creator: Pubkey,
    pub rent_payer: Pubkey,
    // Outer accounts
    pub transaction_buffer: Pubkey,
    pub additional_accounts: Vec<AccountMeta>,
}

pub fn create_transaction_from_buffer_ix(
    accounts: CreateTransactionFromBufferAccounts,
    args: CreateTransactionArgs,
) -> Instruction {
    let mut metas = vec![
        // Inner CreateTransaction accounts (in declaration order).
        AccountMeta::new(accounts.consensus_account, false),
        AccountMeta::new(accounts.transaction, false),
        AccountMeta::new_readonly(accounts.creator, true),
        AccountMeta::new(accounts.rent_payer, true),
        AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        AccountMeta::new_readonly(PROGRAM_ID, false),
        // Outer accounts.
        AccountMeta::new(accounts.transaction_buffer, false),
        AccountMeta::new(accounts.creator, true),
    ];
    metas.extend(accounts.additional_accounts);
    Instruction {
        program_id: PROGRAM_ID,
        accounts: metas,
        data: encode_ix_data("create_transaction_from_buffer", &args),
    }
}

// ============================================================================
// execute_transaction
// ============================================================================

/// Accounts for `execute_transaction`.
///
/// Reads `ctx.remaining_accounts` containing (in order): address lookup table
/// accounts, message account keys, and account-table-lookup accounts (or
/// policy-specific extras for policy execution).
pub struct ExecuteTransactionAccounts {
    pub consensus_account: Pubkey,
    pub proposal: Pubkey,
    pub transaction: Pubkey,
    pub signer: Pubkey,
    pub additional_accounts: Vec<AccountMeta>,
}

pub fn execute_transaction_ix(accounts: ExecuteTransactionAccounts) -> Instruction {
    let mut metas = vec![
        AccountMeta::new(accounts.consensus_account, false),
        AccountMeta::new(accounts.proposal, false),
        AccountMeta::new_readonly(accounts.transaction, false),
        AccountMeta::new_readonly(accounts.signer, true),
        AccountMeta::new_readonly(PROGRAM_ID, false),
    ];
    metas.extend(accounts.additional_accounts);
    Instruction {
        program_id: PROGRAM_ID,
        accounts: metas,
        data: encode_empty_ix_data("execute_transaction"),
    }
}

// ============================================================================
// create_batch
// ============================================================================

pub struct CreateBatchAccounts {
    pub settings: Pubkey,
    pub batch: Pubkey,
    pub creator: Pubkey,
    pub rent_payer: Pubkey,
}

pub fn create_batch_ix(accounts: CreateBatchAccounts, args: CreateBatchArgs) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(accounts.settings, false),
            AccountMeta::new(accounts.batch, false),
            AccountMeta::new_readonly(accounts.creator, true),
            AccountMeta::new(accounts.rent_payer, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: encode_ix_data("create_batch", &args),
    }
}

// ============================================================================
// add_transaction_to_batch
// ============================================================================

pub struct AddTransactionToBatchAccounts {
    pub settings: Pubkey,
    pub proposal: Pubkey,
    pub batch: Pubkey,
    pub transaction: Pubkey,
    pub signer: Pubkey,
    pub rent_payer: Pubkey,
}

pub fn add_transaction_to_batch_ix(
    accounts: AddTransactionToBatchAccounts,
    args: AddTransactionToBatchArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.settings, false),
            AccountMeta::new_readonly(accounts.proposal, false),
            AccountMeta::new(accounts.batch, false),
            AccountMeta::new(accounts.transaction, false),
            AccountMeta::new_readonly(accounts.signer, true),
            AccountMeta::new(accounts.rent_payer, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: encode_ix_data("add_transaction_to_batch", &args),
    }
}

// ============================================================================
// execute_batch_transaction
// ============================================================================

/// Accounts for `execute_batch_transaction`.
///
/// Reads `ctx.remaining_accounts` containing (in order): address lookup table
/// accounts, message account keys, and account-table-lookup accounts.
pub struct ExecuteBatchTransactionAccounts {
    pub settings: Pubkey,
    pub signer: Pubkey,
    pub proposal: Pubkey,
    pub batch: Pubkey,
    pub transaction: Pubkey,
    pub additional_accounts: Vec<AccountMeta>,
}

pub fn execute_batch_transaction_ix(accounts: ExecuteBatchTransactionAccounts) -> Instruction {
    let mut metas = vec![
        AccountMeta::new_readonly(accounts.settings, false),
        AccountMeta::new_readonly(accounts.signer, true),
        AccountMeta::new(accounts.proposal, false),
        AccountMeta::new(accounts.batch, false),
        AccountMeta::new(accounts.transaction, false),
    ];
    metas.extend(accounts.additional_accounts);
    Instruction {
        program_id: PROGRAM_ID,
        accounts: metas,
        data: encode_empty_ix_data("execute_batch_transaction"),
    }
}

// ============================================================================
// create_proposal
// ============================================================================

pub struct CreateProposalAccounts {
    pub consensus_account: Pubkey,
    pub proposal: Pubkey,
    pub creator: Pubkey,
    pub rent_payer: Pubkey,
}

pub fn create_proposal_ix(
    accounts: CreateProposalAccounts,
    args: CreateProposalArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.consensus_account, false),
            AccountMeta::new(accounts.proposal, false),
            AccountMeta::new_readonly(accounts.creator, true),
            AccountMeta::new(accounts.rent_payer, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ],
        data: encode_ix_data("create_proposal", &args),
    }
}

// ============================================================================
// activate_proposal
// ============================================================================

pub struct ActivateProposalAccounts {
    pub settings: Pubkey,
    pub signer: Pubkey,
    pub proposal: Pubkey,
}

pub fn activate_proposal_ix(accounts: ActivateProposalAccounts) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.settings, false),
            AccountMeta::new(accounts.signer, true),
            AccountMeta::new(accounts.proposal, false),
        ],
        data: encode_empty_ix_data("activate_proposal"),
    }
}

// ============================================================================
// Vote on proposal (approve/reject/cancel share `VoteOnProposal` accounts)
// ============================================================================

pub struct VoteOnProposalAccounts {
    pub consensus_account: Pubkey,
    pub signer: Pubkey,
    pub proposal: Pubkey,
    /// Required only for `cancel_proposal` (system_program is `Option` on-chain).
    pub include_system_program: bool,
}

fn vote_on_proposal_metas(accounts: &VoteOnProposalAccounts) -> Vec<AccountMeta> {
    let system_program_meta = if accounts.include_system_program {
        AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false)
    } else {
        AccountMeta::new_readonly(PROGRAM_ID, false)
    };
    vec![
        AccountMeta::new_readonly(accounts.consensus_account, false),
        AccountMeta::new(accounts.signer, true),
        AccountMeta::new(accounts.proposal, false),
        system_program_meta,
        AccountMeta::new_readonly(PROGRAM_ID, false),
    ]
}

pub fn approve_proposal_ix(
    accounts: VoteOnProposalAccounts,
    args: VoteOnProposalArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vote_on_proposal_metas(&accounts),
        data: encode_ix_data("approve_proposal", &args),
    }
}

pub fn reject_proposal_ix(
    accounts: VoteOnProposalAccounts,
    args: VoteOnProposalArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vote_on_proposal_metas(&accounts),
        data: encode_ix_data("reject_proposal", &args),
    }
}

pub fn cancel_proposal_ix(
    accounts: VoteOnProposalAccounts,
    args: VoteOnProposalArgs,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vote_on_proposal_metas(&accounts),
        data: encode_ix_data("cancel_proposal", &args),
    }
}

// ============================================================================
// use_spending_limit
// ============================================================================

/// Accounts for `use_spending_limit`.
///
/// `system_program`, `mint`, `smart_account_token_account`,
/// `destination_token_account`, and `token_program` are optional. Provide them
/// only when transferring SPL tokens (use `mint=None` etc. for SOL transfers).
pub struct UseSpendingLimitAccounts {
    pub settings: Pubkey,
    pub signer: Pubkey,
    pub spending_limit: Pubkey,
    pub smart_account: Pubkey,
    pub destination: Pubkey,
    pub system_program: Option<Pubkey>,
    pub mint: Option<Pubkey>,
    pub smart_account_token_account: Option<Pubkey>,
    pub destination_token_account: Option<Pubkey>,
    pub token_program: Option<Pubkey>,
}

pub fn use_spending_limit_ix(
    accounts: UseSpendingLimitAccounts,
    args: UseSpendingLimitArgs,
) -> Instruction {
    let opt_readonly = |pk: Option<Pubkey>| match pk {
        Some(k) => AccountMeta::new_readonly(k, false),
        None => AccountMeta::new_readonly(PROGRAM_ID, false),
    };
    let opt_writable = |pk: Option<Pubkey>| match pk {
        Some(k) => AccountMeta::new(k, false),
        None => AccountMeta::new_readonly(PROGRAM_ID, false),
    };
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.settings, false),
            AccountMeta::new_readonly(accounts.signer, true),
            AccountMeta::new(accounts.spending_limit, false),
            AccountMeta::new(accounts.smart_account, false),
            AccountMeta::new(accounts.destination, false),
            opt_readonly(accounts.system_program),
            opt_readonly(accounts.mint),
            opt_writable(accounts.smart_account_token_account),
            opt_writable(accounts.destination_token_account),
            opt_readonly(accounts.token_program),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ],
        data: encode_ix_data("use_spending_limit", &args),
    }
}

// ============================================================================
// close_settings_transaction
// ============================================================================

pub struct CloseSettingsTransactionAccounts {
    pub settings: Pubkey,
    pub proposal: Pubkey,
    pub transaction: Pubkey,
    pub proposal_rent_collector: Pubkey,
    pub transaction_rent_collector: Pubkey,
}

pub fn close_settings_transaction_ix(accounts: CloseSettingsTransactionAccounts) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.settings, false),
            AccountMeta::new(accounts.proposal, false),
            AccountMeta::new(accounts.transaction, false),
            AccountMeta::new(accounts.proposal_rent_collector, false),
            AccountMeta::new(accounts.transaction_rent_collector, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ],
        data: encode_empty_ix_data("close_settings_transaction"),
    }
}

// ============================================================================
// close_transaction
// ============================================================================

pub struct CloseTransactionAccounts {
    pub consensus_account: Pubkey,
    pub proposal: Pubkey,
    pub transaction: Pubkey,
    pub proposal_rent_collector: Pubkey,
    pub transaction_rent_collector: Pubkey,
}

pub fn close_transaction_ix(accounts: CloseTransactionAccounts) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.consensus_account, false),
            AccountMeta::new(accounts.proposal, false),
            AccountMeta::new(accounts.transaction, false),
            AccountMeta::new(accounts.proposal_rent_collector, false),
            AccountMeta::new(accounts.transaction_rent_collector, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ],
        data: encode_empty_ix_data("close_transaction"),
    }
}

// ============================================================================
// close_empty_policy_transaction
// ============================================================================

pub struct CloseEmptyPolicyTransactionAccounts {
    pub program_config: Pubkey,
    pub empty_policy: Pubkey,
    pub proposal: Pubkey,
    pub transaction: Pubkey,
    pub proposal_rent_collector: Pubkey,
    pub transaction_rent_collector: Pubkey,
}

pub fn close_empty_policy_transaction_ix(
    accounts: CloseEmptyPolicyTransactionAccounts,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(accounts.program_config, false),
            AccountMeta::new_readonly(accounts.empty_policy, false),
            AccountMeta::new(accounts.proposal, false),
            AccountMeta::new(accounts.transaction, false),
            AccountMeta::new(accounts.proposal_rent_collector, false),
            AccountMeta::new(accounts.transaction_rent_collector, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ],
        data: encode_empty_ix_data("close_empty_policy_transaction"),
    }
}

// ============================================================================
// close_batch_transaction
// ============================================================================

pub struct CloseBatchTransactionAccounts {
    pub settings: Pubkey,
    pub proposal: Pubkey,
    pub batch: Pubkey,
    pub transaction: Pubkey,
    pub transaction_rent_collector: Pubkey,
}

pub fn close_batch_transaction_ix(accounts: CloseBatchTransactionAccounts) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.settings, false),
            AccountMeta::new_readonly(accounts.proposal, false),
            AccountMeta::new(accounts.batch, false),
            AccountMeta::new(accounts.transaction, false),
            AccountMeta::new(accounts.transaction_rent_collector, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: encode_empty_ix_data("close_batch_transaction"),
    }
}

// ============================================================================
// close_batch
// ============================================================================

pub struct CloseBatchAccounts {
    pub settings: Pubkey,
    pub proposal: Pubkey,
    pub batch: Pubkey,
    pub proposal_rent_collector: Pubkey,
    pub batch_rent_collector: Pubkey,
}

pub fn close_batch_ix(accounts: CloseBatchAccounts) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(accounts.settings, false),
            AccountMeta::new(accounts.proposal, false),
            AccountMeta::new(accounts.batch, false),
            AccountMeta::new(accounts.proposal_rent_collector, false),
            AccountMeta::new(accounts.batch_rent_collector, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ],
        data: encode_empty_ix_data("close_batch"),
    }
}

// ============================================================================
// execute_transaction_sync (legacy)
// ============================================================================

/// Accounts for `execute_transaction_sync` (legacy).
///
/// Reads `ctx.remaining_accounts`: first the threshold-many signer accounts
/// (writable as needed), then any remaining accounts the inner instructions
/// need.
pub struct LegacySyncTransactionAccounts {
    pub consensus_account: Pubkey,
    pub additional_accounts: Vec<AccountMeta>,
}

pub fn execute_transaction_sync_ix(
    accounts: LegacySyncTransactionAccounts,
    args: LegacySyncTransactionArgs,
) -> Instruction {
    let mut metas = vec![
        AccountMeta::new_readonly(accounts.consensus_account, false),
        AccountMeta::new_readonly(PROGRAM_ID, false),
    ];
    metas.extend(accounts.additional_accounts);
    Instruction {
        program_id: PROGRAM_ID,
        accounts: metas,
        data: encode_ix_data("execute_transaction_sync", &args),
    }
}

// ============================================================================
// execute_transaction_sync_v2
// ============================================================================

/// Accounts for `execute_transaction_sync_v2`.
///
/// Reads `ctx.remaining_accounts`: signers first, then policy/transaction extras.
pub struct SyncTransactionAccounts {
    pub consensus_account: Pubkey,
    pub additional_accounts: Vec<AccountMeta>,
}

pub fn execute_transaction_sync_v2_ix(
    accounts: SyncTransactionAccounts,
    args: SyncTransactionArgs,
) -> Instruction {
    let mut metas = vec![
        AccountMeta::new(accounts.consensus_account, false),
        AccountMeta::new_readonly(PROGRAM_ID, false),
    ];
    metas.extend(accounts.additional_accounts);
    Instruction {
        program_id: PROGRAM_ID,
        accounts: metas,
        data: encode_ix_data("execute_transaction_sync_v2", &args),
    }
}

// ============================================================================
// execute_settings_transaction_sync
// ============================================================================

/// Accounts for `execute_settings_transaction_sync`.
///
/// Reads `ctx.remaining_accounts`: signers first, then any SpendingLimit
/// accounts to be initialized/closed by the actions.
pub struct SyncSettingsTransactionAccounts {
    pub consensus_account: Pubkey,
    pub rent_payer: Option<Pubkey>,
    pub additional_accounts: Vec<AccountMeta>,
}

pub fn execute_settings_transaction_sync_ix(
    accounts: SyncSettingsTransactionAccounts,
    args: SyncSettingsTransactionArgs,
) -> Instruction {
    let rent_payer_meta = match accounts.rent_payer {
        Some(pk) => AccountMeta::new(pk, true),
        None => AccountMeta::new_readonly(PROGRAM_ID, false),
    };
    let system_program_meta = match accounts.rent_payer {
        Some(_) => AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        None => AccountMeta::new_readonly(PROGRAM_ID, false),
    };
    let mut metas = vec![
        AccountMeta::new(accounts.consensus_account, false),
        rent_payer_meta,
        system_program_meta,
        AccountMeta::new_readonly(PROGRAM_ID, false),
    ];
    metas.extend(accounts.additional_accounts);
    Instruction {
        program_id: PROGRAM_ID,
        accounts: metas,
        data: encode_ix_data("execute_settings_transaction_sync", &args),
    }
}

// ============================================================================
// log_event
// ============================================================================

/// Accounts for `log_event`.
///
/// `log_authority` is a signer with non-zero data owned by the program.
/// `additional_accounts` are forwarded as `ctx.remaining_accounts`.
pub struct LogEventAccounts {
    pub log_authority: Pubkey,
    pub additional_accounts: Vec<AccountMeta>,
}

pub fn log_event_ix(accounts: LogEventAccounts, args: LogEventArgsV2) -> Instruction {
    let mut metas = vec![AccountMeta::new_readonly(accounts.log_authority, true)];
    metas.extend(accounts.additional_accounts);
    Instruction {
        program_id: PROGRAM_ID,
        accounts: metas,
        data: encode_ix_data("log_event", &args),
    }
}
