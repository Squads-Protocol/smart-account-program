use solana_address::Address;

use crate::errors::SmartAccountError;

pub const MAX_TIME_LOCK: u32 = 3 * 30 * 24 * 60 * 60; // 3 months

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub struct Settings {
    /// An integer that is used seed the settings PDA. Its incremented by 1
    /// inside the program conifg by 1 for each smart account created. This is
    /// to ensure uniqueness of each settings PDA without relying on user input.
    ///
    /// Note: As this represents a DOS vector in the current creation architecture,
    /// account creation will be permissioned until compression is implemented.
    pub seed: u128,
    /// The authority that can change the smart account settings.
    /// This is a very important parameter as this authority can change the signers and threshold.
    pub settings_authority: Address,
    /// Threshold for signatures.
    pub threshold: u16,
    /// How many seconds must pass between transaction voting settlement and execution.
    pub time_lock: u32,
    /// Last transaction index. 0 means no transactions have been created.
    pub transaction_index: u64,
    /// Last stale transaction index. All transactions up until this index are stale.
    pub stale_transaction_index: u64,
    /// Field reserved for when archival/compression is implemented.
    pub archival_authority: Option<Address>,
    /// Field that will prevent a smart account from being archived immediately after unarchival.
    pub archivable_after: u64,
    /// Bump for the smart account PDA seed.
    pub bump: u8,
    /// Signers attached to the smart account
    pub signers: Vec<SmartAccountSigner>,
    /// Counter for how many sub accounts are in use (improves off-chain indexing)
    pub account_utilization: u8,
    /// Seed used for deterministic policy creation.
    pub policy_seed: Option<u64>,
    // Reserved for future use
    pub _reserved2: u8,
}

impl Settings {
    pub const DISCRIMINATOR: [u8; 8] = [0xdf, 0xb3, 0xa3, 0xbe, 0xb1, 0xe0, 0x43, 0xad];

    pub fn size(signers_length: usize) -> usize {
        8  + // anchor account discriminator
        16 + // seed
        32 + // settings_authority
        2  + // threshold
        4  + // time_lock
        8  + // transaction_index
        8  + // stale_transaction_index
        1  + // archival_authority Option discriminator
        32 + // archival_authority (always 32 bytes, even if None, just to keep the realloc logic simpler)
        8  + // archivable_after
        1  + // bump
        4  + // signers vector length
        signers_length * SmartAccountSigner::INIT_SPACE + // signers
        1  + // sub_account_utilization
        1  + 8 + // policy_seed
        1 // _reserved_2
    }

    /// Validate the settings state. Must be called at the end of every instruction that mutates Settings.
    pub fn invariant(&self) -> Result<(), SmartAccountError> {
        let Self {
            threshold,
            signers,
            transaction_index,
            stale_transaction_index,
            ..
        } = self;

        if signers.len() > usize::from(u16::MAX) {
            return Err(SmartAccountError::TooManySigners);
        }

        let has_duplicates = signers.windows(2).any(|win| win[0].key == win[1].key);
        if has_duplicates {
            return Err(SmartAccountError::DuplicateSigner);
        }

        if !signers.iter().all(|m| m.permissions.mask < 8) {
            return Err(SmartAccountError::UnknownPermission);
        }

        let num_proposers = self.num_proposers();
        if num_proposers == 0 {
            return Err(SmartAccountError::NoProposers);
        }

        let num_executors = self.num_executors();
        if num_executors == 0 {
            return Err(SmartAccountError::NoExecutors);
        }

        let num_voters = self.num_voters();
        if num_voters == 0 {
            return Err(SmartAccountError::NoVoters);
        }

        if *threshold == 0 {
            return Err(SmartAccountError::InvalidThreshold);
        }

        if usize::from(*threshold) > num_voters {
            return Err(SmartAccountError::InvalidThreshold);
        }

        if stale_transaction_index > transaction_index {
            return Err(SmartAccountError::InvalidStaleTransactionIndex);
        }

        if self.time_lock > MAX_TIME_LOCK {
            return Err(SmartAccountError::TimeLockExceedsMaxAllowed);
        }

        Ok(())
    }

    /// Add `new_signer` to the settings `signers` vec and sort the vec.
    pub fn add_signer(&mut self, new_signer: SmartAccountSigner) {
        self.signers.push(new_signer);
        self.signers.sort_by_key(|m| m.key);
    }

    /// Remove `signer_pubkey` from the settings `signers` vec.
    pub fn remove_signer(&mut self, signer_pubkey: Address) -> Result<(), SmartAccountError> {
        let old_signer_index = match self.is_signer(signer_pubkey) {
            Some(old_signer_index) => old_signer_index,
            None => return Err(SmartAccountError::NotASigner),
        };
        self.signers.remove(old_signer_index);
        Ok(())
    }

    pub fn increment_account_utilization(&mut self) {
        self.account_utilization = self.account_utilization.checked_add(1).unwrap();
    }

    /// Returns `Some(index)` if `signer_pubkey` is a signer, with `index` into the `signers` vec.
    pub fn is_signer(&self, signer_pubkey: Address) -> Option<usize> {
        self.signers
            .binary_search_by_key(&signer_pubkey, |s| s.key)
            .ok()
    }

    pub fn num_voters(&self) -> usize {
        self.signers
            .iter()
            .filter(|s| s.permissions.has(Permission::Vote))
            .count()
    }

    pub fn num_proposers(&self) -> usize {
        self.signers
            .iter()
            .filter(|s| s.permissions.has(Permission::Initiate))
            .count()
    }

    pub fn num_executors(&self) -> usize {
        self.signers
            .iter()
            .filter(|s| s.permissions.has(Permission::Execute))
            .count()
    }

    pub fn invalidate_prior_transactions(&mut self) {
        self.stale_transaction_index = self.transaction_index;
    }
}

#[cfg(feature = "borsh")]
impl Settings {
    pub fn try_deserialize(data: &[u8]) -> Result<Self, borsh::io::Error> {
        if data.len() < 8 {
            return Err(borsh::io::Error::new(
                borsh::io::ErrorKind::InvalidData,
                "account data shorter than discriminator",
            ));
        }
        if data[..8] != Self::DISCRIMINATOR {
            return Err(borsh::io::Error::new(
                borsh::io::ErrorKind::InvalidData,
                "discriminator mismatch",
            ));
        }
        let mut body = &data[8..];
        <Self as borsh::BorshDeserialize>::deserialize(&mut body)
    }
}

#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Eq, PartialEq, Clone)]
pub struct SmartAccountSigner {
    pub key: Address,
    pub permissions: Permissions,
}

impl SmartAccountSigner {
    pub const INIT_SPACE: usize = 32 + Permissions::INIT_SPACE;
}

#[derive(Clone, Copy)]
pub enum Permission {
    Initiate = 1 << 0,
    Vote = 1 << 1,
    Execute = 1 << 2,
}

/// Bitmask for permissions.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Eq, PartialEq, Clone, Copy, Default, Debug)]
pub struct Permissions {
    pub mask: u8,
}

impl Permissions {
    pub const INIT_SPACE: usize = 1;

    pub fn from_vec(permissions: &[Permission]) -> Self {
        let mut mask = 0;
        for permission in permissions {
            mask |= *permission as u8;
        }
        Self { mask }
    }

    pub fn has(&self, permission: Permission) -> bool {
        self.mask & (permission as u8) != 0
    }

    pub fn all() -> Self {
        Self { mask: 0b111 }
    }
}
