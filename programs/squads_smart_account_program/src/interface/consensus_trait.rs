use anchor_lang::prelude::*;

use crate::{errors::SmartAccountError, Permission, Permissions};

pub trait ConsensusSigner {
    fn key(&self) -> Pubkey;
    fn permissions(&self) -> Permissions;
}

pub enum ConsensusAccountType {
    Settings,
    Policy,
}

pub trait Consensus {
    type SignerType: ConsensusSigner;

    fn account_type(&self) -> ConsensusAccountType;
    fn key(&self) -> Pubkey;
    fn check_derivation(&self) -> Result<()>;

    // Core consensus fields
    fn signers(&self) -> &[Self::SignerType];
    fn threshold(&self) -> u16;
    fn time_lock(&self) -> u32;
    fn transaction_index(&self) -> u64;
    fn set_transaction_index(&mut self, transaction_index: u64) -> Result<()>;
    fn stale_transaction_index(&self) -> u64;

    // Signer validation methods (ported from Settings)
    fn is_signer(&self, signer_pubkey: Pubkey) -> Option<usize> {
        self.signers()
            .binary_search_by_key(&signer_pubkey, |s| s.key())
            .ok()
    }

    fn signer_has_permission(&self, signer_pubkey: Pubkey, permission: Permission) -> bool {
        match self.is_signer(signer_pubkey) {
            Some(index) => self.signers()[index].permissions().has(permission),
            _ => false,
        }
    }

    // Permission counting methods (ported from Settings)
    fn num_voters(&self) -> usize {
        self.signers()
            .iter()
            .filter(|s| s.permissions().has(Permission::Vote))
            .count()
    }

    fn num_proposers(&self) -> usize {
        self.signers()
            .iter()
            .filter(|s| s.permissions().has(Permission::Initiate))
            .count()
    }

    fn num_executors(&self) -> usize {
        self.signers()
            .iter()
            .filter(|s| s.permissions().has(Permission::Execute))
            .count()
    }

    // Rejection cutoff calculation (ported from Settings)
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
    fn invariant(&self) -> Result<()> {
        // Max number of signers is u16::MAX
        require!(
            self.signers().len() <= usize::from(u16::MAX),
            SmartAccountError::TooManySigners
        );

        // No duplicate signers (assumes sorted)
        let has_duplicates = self
            .signers()
            .windows(2)
            .any(|win| win[0].key() == win[1].key());
        require!(!has_duplicates, SmartAccountError::DuplicateSigner);

        // Signers must not have unknown permissions
        require!(
            self.signers().iter().all(|s| s.permissions().mask < 8),
            SmartAccountError::UnknownPermission
        );

        // Must have at least one signer with each permission
        require!(self.num_proposers() > 0, SmartAccountError::NoProposers);
        require!(self.num_executors() > 0, SmartAccountError::NoExecutors);
        require!(self.num_voters() > 0, SmartAccountError::NoVoters);

        // Threshold validation
        require!(self.threshold() > 0, SmartAccountError::InvalidThreshold);
        require!(
            usize::from(self.threshold()) <= self.num_voters(),
            SmartAccountError::InvalidThreshold
        );

        // Stale transaction index validation
        require!(
            self.stale_transaction_index() <= self.transaction_index(),
            SmartAccountError::InvalidStaleTransactionIndex
        );

        Ok(())
    }
}
