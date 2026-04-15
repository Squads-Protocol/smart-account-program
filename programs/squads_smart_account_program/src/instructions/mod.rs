// Anchor boilerplate for ResolvedSigner composite field.
// Must be `mod` declarations here (not re-exports) so the derive macro
// finds them via `use super::*` inside generated client modules.
pub(crate) mod __client_accounts_resolved_signer {
    use anchor_lang::solana_program::{instruction::AccountMeta, pubkey::Pubkey};
    use anchor_lang::prelude::borsh;

    #[derive(borsh::BorshSerialize)]
    pub struct ResolvedSigner {
        pub info: Pubkey,
    }

    impl anchor_lang::ToAccountMetas for ResolvedSigner {
        fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
            let is_signer = is_signer.unwrap_or_default();
            vec![AccountMeta::new_readonly(self.info, is_signer)]
        }
    }
}

pub(crate) mod __cpi_client_accounts_resolved_signer {
    use anchor_lang::solana_program::{account_info::AccountInfo, instruction::AccountMeta};

    pub struct ResolvedSigner<'info> {
        pub info: AccountInfo<'info>,
    }

    impl<'info> anchor_lang::ToAccountMetas for ResolvedSigner<'info> {
        fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
            let is_signer = is_signer.unwrap_or(self.info.is_signer);
            let meta = match self.info.is_writable {
                false => AccountMeta::new_readonly(*self.info.key, is_signer),
                true => AccountMeta::new(*self.info.key, is_signer),
            };
            vec![meta]
        }
    }

    impl<'info> anchor_lang::ToAccountInfos<'info> for ResolvedSigner<'info> {
        fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
            vec![self.info.clone()]
        }
    }
}

pub use activate_proposal::*;
pub use create_session_key::*;
pub use increment_account_index::*;
pub use authority_settings_transaction_execute::*;
pub use authority_spending_limit_add::*;
pub use authority_spending_limit_remove::*;
pub use batch_add_transaction::*;
pub use batch_create::*;
pub use batch_execute_transaction::*;
pub use log_event::*;
pub use program_config_change::*;
pub use program_config_init::*;
pub use proposal_create::*;
pub use proposal_vote::*;
pub use settings_transaction_create::*;
pub use settings_transaction_execute::*;
pub use settings_transaction_sync::*;
pub use smart_account_create::*;
pub use transaction_buffer_close::*;
pub use transaction_buffer_create::*;
pub use transaction_buffer_extend::*;
pub use transaction_close::*;
pub use transaction_create::*;
pub use transaction_create_from_buffer::*;
pub use transaction_execute::*;
pub use transaction_execute_sync::*;
pub use transaction_execute_sync_legacy::*;
pub use use_spending_limit::*;
pub use revoke_session_key::*;

mod activate_proposal;
mod create_session_key;
mod increment_account_index;
mod revoke_session_key;
mod authority_settings_transaction_execute;
mod authority_spending_limit_add;
mod authority_spending_limit_remove;
mod batch_add_transaction;
mod batch_create;
mod batch_execute_transaction;
mod log_event;
mod program_config_change;
mod program_config_init;
mod proposal_create;
mod proposal_vote;
mod settings_transaction_create;
mod settings_transaction_execute;
mod settings_transaction_sync;
mod smart_account_create;
mod transaction_buffer_close;
mod transaction_buffer_create;
mod transaction_buffer_extend;
mod transaction_close;
mod transaction_create;
mod transaction_create_from_buffer;
mod transaction_execute;
mod transaction_execute_sync;
mod transaction_execute_sync_legacy;
mod use_spending_limit;
