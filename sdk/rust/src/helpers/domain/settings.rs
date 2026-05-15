use solana_address::Address;

use crate::generated::accounts::Settings;
use crate::generated::errors::SquadsSmartAccountProgramError as Error;
use crate::generated::types::SmartAccountSigner;
use crate::helpers::permissions::{Permission, PermissionsExt};

/// Three months, matching the on-chain constraint.
pub const MAX_TIME_LOCK: u32 = 3 * 30 * 24 * 60 * 60;

/// Bytes consumed by a `SmartAccountSigner` on-chain.
pub const SMART_ACCOUNT_SIGNER_INIT_SPACE: usize = 32 + 1;

pub trait SettingsExt {
    /// Anchor account size for a `Settings` with `signers_length` signers.
    fn size(signers_length: usize) -> usize;
    fn invariant(&self) -> Result<(), Error>;
    fn add_signer(&mut self, new_signer: SmartAccountSigner);
    fn remove_signer(&mut self, signer_pubkey: Address) -> Result<(), Error>;
    fn is_signer(&self, signer_pubkey: Address) -> Option<usize>;
    fn num_voters(&self) -> usize;
    fn num_proposers(&self) -> usize;
    fn num_executors(&self) -> usize;
    fn invalidate_prior_transactions(&mut self);
    fn increment_account_utilization(&mut self);
}

impl SettingsExt for Settings {
    fn size(signers_length: usize) -> usize {
        8  // discriminator
        + 16  // seed
        + 32  // settings_authority
        + 2   // threshold
        + 4   // time_lock
        + 8   // transaction_index
        + 8   // stale_transaction_index
        + 1 + 32  // Option<Address> archival_authority — anchor reserves 32 bytes
        + 8   // archivable_after
        + 1   // bump
        + 4   // signers vec length prefix
        + signers_length * SMART_ACCOUNT_SIGNER_INIT_SPACE
        + 1   // account_utilization
        + 1   // reserved1
        + 1 // reserved2
    }

    fn invariant(&self) -> Result<(), Error> {
        if self.signers.len() > usize::from(u16::MAX) {
            return Err(Error::TooManySigners);
        }
        let has_duplicates = self.signers.windows(2).any(|w| w[0].key == w[1].key);
        if has_duplicates {
            return Err(Error::DuplicateSigner);
        }
        if !self.signers.iter().all(|s| s.permissions.mask < 8) {
            return Err(Error::UnknownPermission);
        }
        if self.num_proposers() == 0 {
            return Err(Error::NoProposers);
        }
        if self.num_executors() == 0 {
            return Err(Error::NoExecutors);
        }
        let voters = self.num_voters();
        if voters == 0 {
            return Err(Error::NoVoters);
        }
        if self.threshold == 0 || usize::from(self.threshold) > voters {
            return Err(Error::InvalidThreshold);
        }
        if self.stale_transaction_index > self.transaction_index {
            return Err(Error::InvalidStaleTransactionIndex);
        }
        if self.time_lock > MAX_TIME_LOCK {
            return Err(Error::TimeLockExceedsMaxAllowed);
        }
        Ok(())
    }

    fn add_signer(&mut self, new_signer: SmartAccountSigner) {
        self.signers.push(new_signer);
        self.signers.sort_by_key(|s| s.key);
    }

    fn remove_signer(&mut self, signer_pubkey: Address) -> Result<(), Error> {
        let idx = self.is_signer(signer_pubkey).ok_or(Error::NotASigner)?;
        self.signers.remove(idx);
        Ok(())
    }

    fn is_signer(&self, signer_pubkey: Address) -> Option<usize> {
        self.signers
            .binary_search_by_key(&signer_pubkey, |s| s.key)
            .ok()
    }

    fn num_voters(&self) -> usize {
        self.signers
            .iter()
            .filter(|s| s.permissions.has(Permission::Vote))
            .count()
    }

    fn num_proposers(&self) -> usize {
        self.signers
            .iter()
            .filter(|s| s.permissions.has(Permission::Initiate))
            .count()
    }

    fn num_executors(&self) -> usize {
        self.signers
            .iter()
            .filter(|s| s.permissions.has(Permission::Execute))
            .count()
    }

    fn invalidate_prior_transactions(&mut self) {
        self.stale_transaction_index = self.transaction_index;
    }

    fn increment_account_utilization(&mut self) {
        self.account_utilization = self.account_utilization.saturating_add(1);
    }
}
