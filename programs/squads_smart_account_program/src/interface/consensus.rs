use anchor_lang::{
    prelude::{AccountInfo, Pubkey},
    AccountDeserialize, AccountSerialize, Discriminator, Owners, Result,
};

use crate::{
    errors::SmartAccountError, get_policy_signer_seeds, get_settings_signer_seeds, state::{Policy, Settings}, SmartAccountSigner
};

use super::consensus_trait::{Consensus, ConsensusAccountType};

#[derive(Clone)]
pub enum ConsensusAccount {
    Settings(Settings),
    Policy(Policy),
}

static OWNERS: [Pubkey; 1] = [crate::ID];

// Implemented for InterfaceAccount
impl Owners for ConsensusAccount {
    // Just our own program ID, since the interface account is just to wrap the trait
    fn owners() -> &'static [Pubkey] {
        &OWNERS
    }
}

// Implemented for InterfaceAccount
impl AccountSerialize for ConsensusAccount {
    fn try_serialize<W: std::io::Write>(&self, writer: &mut W) -> anchor_lang::Result<()> {
        match self {
            ConsensusAccount::Settings(settings) => settings.try_serialize(writer),
            ConsensusAccount::Policy(policy) => policy.try_serialize(writer),
        }
    }
}

// Implemented for InterfaceAccount
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
    /// Returns the number of signers in the consensus account.
    pub fn signers_len(&self) -> usize {
        self.signers().len()
    }

    /// Returns the bump for the consensus account.
    pub fn bump(&self) -> u8 {
        match self {
            ConsensusAccount::Settings(settings) => settings.bump,
            ConsensusAccount::Policy(policy) => policy.bump,
        }
    }
    /// Returns the signer seeds for the consensus account.
    pub fn get_signer_seeds(&self) -> Vec<Vec<u8>> {
        match self {
            ConsensusAccount::Settings(settings) => get_settings_signer_seeds(settings.seed),
            ConsensusAccount::Policy(policy) => get_policy_signer_seeds(&policy.settings, policy.seed),
        }
    }
    /// Returns the settings if the consensus account is a settings.
    pub fn settings(&mut self) -> Result<&mut Settings> {
        match self {
            ConsensusAccount::Settings(settings) => Ok(settings),
            ConsensusAccount::Policy(_) => {
                Err(SmartAccountError::ConsensusAccountNotSettings.into())
            }
        }
    }

    /// Returns the settings if the consensus account is a settings.
    pub fn read_only_settings(&self) -> Result<&Settings> {
        match self {
            ConsensusAccount::Settings(settings) => Ok(settings),
            ConsensusAccount::Policy(_) => {
                Err(SmartAccountError::ConsensusAccountNotSettings.into())
            }
        }
    }

    /// Returns the policy if the consensus account is a policy.
    pub fn policy(&mut self) -> Result<&mut Policy> {
        match self {
            ConsensusAccount::Settings(_) => {
                return Err(SmartAccountError::ConsensusAccountNotPolicy.into())
            }
            ConsensusAccount::Policy(policy) => Ok(policy),
        }
    }

    /// Returns the policy if the consensus account is a policy.
    pub fn read_only_policy(&self) -> Result<&Policy> {
        match self {
            ConsensusAccount::Settings(_) => {
                return Err(SmartAccountError::ConsensusAccountNotPolicy.into())
            }
            ConsensusAccount::Policy(policy) => Ok(policy),
        }
    }

    /// Helper method to delegate to the underlying consensus implementation
    fn as_consensus(&self) -> &dyn Consensus {
        match self {
            ConsensusAccount::Settings(settings) => settings,
            ConsensusAccount::Policy(policy) => policy,
        }
    }

    /// Helper method to delegate to the underlying consensus implementation (mutable)
    fn as_consensus_mut(&mut self) -> &mut dyn Consensus {
        match self {
            ConsensusAccount::Settings(settings) => settings,
            ConsensusAccount::Policy(policy) => policy,
        }
    }
}

impl Consensus for ConsensusAccount {
    fn is_active(&self, accounts: &[AccountInfo]) -> Result<()> {
        self.as_consensus().is_active(accounts)
    }

    fn check_derivation(&self, key: Pubkey) -> Result<()> {
        self.as_consensus().check_derivation(key)
    }

    fn account_type(&self) -> ConsensusAccountType {
        self.as_consensus().account_type()
    }

    fn signers(&self) -> &[SmartAccountSigner] {
        self.as_consensus().signers()
    }

    fn threshold(&self) -> u16 {
        self.as_consensus().threshold()
    }

    fn time_lock(&self) -> u32 {
        self.as_consensus().time_lock()
    }

    fn transaction_index(&self) -> u64 {
        self.as_consensus().transaction_index()
    }

    fn set_transaction_index(&mut self, transaction_index: u64) -> Result<()> {
        self.as_consensus_mut()
            .set_transaction_index(transaction_index)
    }

    fn stale_transaction_index(&self) -> u64 {
        self.as_consensus().stale_transaction_index()
    }

    fn invalidate_prior_transactions(&mut self) {
        self.as_consensus_mut().invalidate_prior_transactions()
    }

    fn is_signer(&self, signer_pubkey: Pubkey) -> Option<usize> {
        self.as_consensus().is_signer(signer_pubkey)
    }

    fn signer_has_permission(&self, signer_pubkey: Pubkey, permission: crate::Permission) -> bool {
        self.as_consensus()
            .signer_has_permission(signer_pubkey, permission)
    }

    fn num_voters(&self) -> usize {
        self.as_consensus().num_voters()
    }

    fn num_proposers(&self) -> usize {
        self.as_consensus().num_proposers()
    }

    fn num_executors(&self) -> usize {
        self.as_consensus().num_executors()
    }

    fn cutoff(&self) -> usize {
        self.as_consensus().cutoff()
    }

    fn invariant(&self) -> Result<()> {
        self.as_consensus().invariant()
    }
}
