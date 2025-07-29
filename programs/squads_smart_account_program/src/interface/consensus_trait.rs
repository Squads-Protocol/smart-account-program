use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{errors::SmartAccountError, Permission, SmartAccountSigner};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum ConsensusAccountType {
    Settings,
    Policy,
}

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
    /// `None` otherwise.
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
    /// The cutoff must be such that it is impossible for the remaining voters to reach the approval threshold.
    /// For example: total voters = 7, threshold = 3, cutoff = 5.
    fn cutoff(&self) -> usize {
        self.num_voters()
            .checked_sub(usize::from(self.threshold()))
            .unwrap()
            .checked_add(1)
            .unwrap()
    }

    // Stale transaction protection (ported from Settings)
    fn invalidate_prior_transactions(&mut self);

    // Consensus validation (ported from Settings invariant)
    fn invariant(&self) -> Result<()>;
}
