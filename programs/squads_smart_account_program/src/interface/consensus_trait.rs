//! The `Consensus` trait (used by `InterfaceAccount<'info, ConsensusAccount>`
//! in the program). `ConsensusAccountType` re-exported from the types crate.

use anchor_lang::prelude::*;

pub use squads_smart_account_program_types::ConsensusAccountType;

use crate::{Permission, SmartAccountSigner};

pub trait Consensus {
    fn account_type(&self) -> ConsensusAccountType;
    fn check_derivation(&self, key: Pubkey) -> Result<()>;
    fn is_active(&self, accounts: &[AccountInfo]) -> Result<()>;

    // Core consensus fields
    fn signers(&self) -> &[SmartAccountSigner];
    fn threshold(&self) -> u16;
    fn time_lock(&self) -> u32;
    fn transaction_index(&self) -> u64;
    fn set_transaction_index(&mut self, transaction_index: u64) -> Result<()>;
    fn stale_transaction_index(&self) -> u64;

    // Returns `Some(index)` if `signer_pubkey` is a signer, with `index` into the `signers` vec.
    fn is_signer(&self, signer_pubkey: Pubkey) -> Option<usize> {
        self.signers()
            .binary_search_by_key(&signer_pubkey, |s| s.key)
            .ok()
    }

    fn signer_has_permission(&self, signer_pubkey: Pubkey, permission: Permission) -> bool {
        match self.is_signer(signer_pubkey) {
            Some(index) => self.signers()[index].permissions.has(permission),
            _ => false,
        }
    }

    // Permission counting methods
    fn num_voters(&self) -> usize {
        self.signers()
            .iter()
            .filter(|s| s.permissions.has(Permission::Vote))
            .count()
    }

    fn num_proposers(&self) -> usize {
        self.signers()
            .iter()
            .filter(|s| s.permissions.has(Permission::Initiate))
            .count()
    }

    fn num_executors(&self) -> usize {
        self.signers()
            .iter()
            .filter(|s| s.permissions.has(Permission::Execute))
            .count()
    }

    /// How many "reject" votes are enough to make the transaction "Rejected".
    fn cutoff(&self) -> usize {
        self.num_voters()
            .checked_sub(usize::from(self.threshold()))
            .unwrap()
            .checked_add(1)
            .unwrap()
    }

    // Stale transaction protection
    fn invalidate_prior_transactions(&mut self);

    // Consensus validation
    fn invariant(&self) -> Result<()>;
}
