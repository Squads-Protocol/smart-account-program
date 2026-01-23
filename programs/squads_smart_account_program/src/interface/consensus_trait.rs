use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{Permission, SmartAccountSigner, SmartAccountSignerWrapper};

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
    fn signers(&self) -> &SmartAccountSignerWrapper;
    fn threshold(&self) -> u16;
    fn time_lock(&self) -> u32;
    fn transaction_index(&self) -> u64;
    fn set_transaction_index(&mut self, transaction_index: u64) -> Result<()>;
    fn stale_transaction_index(&self) -> u64;

    /// Get signers as V2 format (canonical view for consensus)
    fn signers_v2(&self) -> Vec<SmartAccountSigner> {
        self.signers().as_v2()
    }

    /// Number of signers
    fn signers_len(&self) -> usize {
        self.signers().len()
    }

    /// Check if signer exists (by key or key_id)
    fn is_signer_v2(&self, key: Pubkey) -> Option<SmartAccountSigner> {
        self.signers().find(&key)
    }

    /// Find an external signer by their active session key.
    /// Returns the signer if the pubkey matches an active session key.
    fn find_signer_by_session_key(&self, pubkey: Pubkey, current_timestamp: u64) -> Option<SmartAccountSigner> {
        self.signers().find_by_session_key(&pubkey, current_timestamp)
    }

    /// Returns `Some(index)` if `signer_pubkey` is a signer, with `index` into the `signers` vec.
    /// `None` otherwise.
    fn is_signer(&self, signer_pubkey: Pubkey) -> Option<usize> {
        self.signers().find_index(&signer_pubkey)
    }

    fn signer_has_permission(&self, signer_pubkey: Pubkey, permission: Permission) -> bool {
        match self.is_signer_v2(signer_pubkey) {
            Some(signer) => signer.permissions().has(permission),
            _ => false,
        }
    }

    // Permission counting methods
    fn num_voters(&self) -> usize {
        self.signers().count_with_permission(Permission::Vote)
    }

    fn num_proposers(&self) -> usize {
        self.signers().count_with_permission(Permission::Initiate)
    }

    fn num_executors(&self) -> usize {
        self.signers().count_with_permission(Permission::Execute)
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
