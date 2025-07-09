use anchor_lang::{
    prelude::Pubkey, AccountDeserialize, AccountSerialize, Discriminator, Owners, Result,
};

use crate::{state::Settings, Policy};

use super::consensus_trait::{Consensus, ConsensusAccountType};

#[derive(Clone)]
pub enum ConsensusAccount {
    Settings(Settings),
    Policy(Policy),
}

static OWNERS: [Pubkey; 1] = [crate::ID];

impl Owners for ConsensusAccount {
    // Just our own program ID, since the interface account is just to wrap the trait
    fn owners() -> &'static [Pubkey] {
        &OWNERS
    }
}

impl AccountSerialize for ConsensusAccount {
    fn try_serialize<W: std::io::Write>(&self, writer: &mut W) -> anchor_lang::Result<()> {
        match self {
            ConsensusAccount::Settings(settings) => settings.try_serialize(writer),
            ConsensusAccount::Policy(policy) => policy.try_serialize(writer),
        }
    }
}

impl AccountDeserialize for ConsensusAccount {
    fn try_deserialize(reader: &mut &[u8]) -> anchor_lang::Result<Self> {
        let discriminator: [u8; 8] = reader[..8].try_into().unwrap();
        match discriminator {
            Settings::DISCRIMINATOR => Ok(ConsensusAccount::Settings(Settings::try_deserialize(
                reader,
            )?)),
            Policy::DISCRIMINATOR => Ok(ConsensusAccount::Policy(Policy::try_deserialize(reader)?)),
            _ => Err(anchor_lang::error::Error::from(
                anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch,
            )),
        }
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
        let discriminator: [u8; 8] = buf[..8].try_into().unwrap();
        match discriminator {
            Settings::DISCRIMINATOR => Ok(ConsensusAccount::Settings(
                Settings::try_deserialize_unchecked(buf)?,
            )),
            Policy::DISCRIMINATOR => Ok(ConsensusAccount::Policy(
                Policy::try_deserialize_unchecked(buf)?,
            )),
            _ => Err(anchor_lang::error::Error::from(
                anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch,
            )),
        }
    }
}

impl ConsensusAccount {
    pub fn check_derivation(&self) -> Result<()> {
        match self {
            ConsensusAccount::Settings(settings) => settings.check_derivation()?,
            ConsensusAccount::Policy(policy) => policy.check_derivation()?,
        }
        Ok(())
    }

    // Helper methods that delegate to the underlying consensus implementations
    pub fn account_type(&self) -> ConsensusAccountType {
        match self {
            ConsensusAccount::Settings(settings) => settings.account_type(),
            ConsensusAccount::Policy(policy) => policy.account_type(),
        }
    }

    pub fn signers_len(&self) -> usize {
        match self {
            ConsensusAccount::Settings(settings) => settings.signers().len(),
            ConsensusAccount::Policy(policy) => policy.signers().len(),
        }
    }

    pub fn key(&self) -> Pubkey {
        match self {
            ConsensusAccount::Settings(settings) => settings.key(),
            ConsensusAccount::Policy(policy) => policy.key(),
        }
    }

    pub fn threshold(&self) -> u16 {
        match self {
            ConsensusAccount::Settings(settings) => settings.threshold(),
            ConsensusAccount::Policy(policy) => policy.threshold(),
        }
    }

    pub fn time_lock(&self) -> u32 {
        match self {
            ConsensusAccount::Settings(settings) => settings.time_lock(),
            ConsensusAccount::Policy(policy) => policy.time_lock(),
        }
    }

    pub fn transaction_index(&self) -> u64 {
        match self {
            ConsensusAccount::Settings(settings) => settings.transaction_index(),
            ConsensusAccount::Policy(policy) => policy.transaction_index(),
        }
    }

    pub fn set_transaction_index(&mut self, transaction_index: u64) -> Result<()> {
        match self {
            ConsensusAccount::Settings(settings) => settings.set_transaction_index(transaction_index),
            ConsensusAccount::Policy(policy) => policy.set_transaction_index(transaction_index),
        }
    }

    pub fn stale_transaction_index(&self) -> u64 {
        match self {
            ConsensusAccount::Settings(settings) => settings.stale_transaction_index(),
            ConsensusAccount::Policy(policy) => policy.stale_transaction_index(),
        }
    }

    pub fn invalidate_prior_transactions(&mut self) {
        match self {
            ConsensusAccount::Settings(settings) => settings.invalidate_prior_transactions(),
            ConsensusAccount::Policy(policy) => policy.invalidate_prior_transactions(),
        }
    }

    // Delegate consensus helper methods
    pub fn is_signer(&self, signer_pubkey: Pubkey) -> Option<usize> {
        match self {
            ConsensusAccount::Settings(settings) => settings.is_signer(signer_pubkey),
            ConsensusAccount::Policy(policy) => policy.is_signer(signer_pubkey),
        }
    }

    pub fn signer_has_permission(
        &self,
        signer_pubkey: Pubkey,
        permission: crate::Permission,
    ) -> bool {
        match self {
            ConsensusAccount::Settings(settings) => {
                settings.signer_has_permission(signer_pubkey, permission)
            }
            ConsensusAccount::Policy(policy) => {
                policy.signer_has_permission(signer_pubkey, permission)
            }
        }
    }

    pub fn num_voters(&self) -> usize {
        match self {
            ConsensusAccount::Settings(settings) => settings.num_voters(),
            ConsensusAccount::Policy(policy) => policy.num_voters(),
        }
    }

    pub fn num_proposers(&self) -> usize {
        match self {
            ConsensusAccount::Settings(settings) => settings.num_proposers(),
            ConsensusAccount::Policy(policy) => policy.num_proposers(),
        }
    }

    pub fn num_executors(&self) -> usize {
        match self {
            ConsensusAccount::Settings(settings) => settings.num_executors(),
            ConsensusAccount::Policy(policy) => policy.num_executors(),
        }
    }

    pub fn cutoff(&self) -> usize {
        match self {
            ConsensusAccount::Settings(settings) => settings.cutoff(),
            ConsensusAccount::Policy(policy) => policy.cutoff(),
        }
    }

    pub fn invariant(&self) -> anchor_lang::Result<()> {
        match self {
            ConsensusAccount::Settings(settings) => settings.invariant(),
            ConsensusAccount::Policy(policy) => policy.invariant(),
        }
    }
}
