use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{errors::SmartAccountError, Permission, SmartAccountSigner, SmartAccountSignerWrapper};
use crate::state::SignerType;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::{
    split_instructions_sysvar,
    verify_precompile_signers,
};
use crate::utils::context_validation::verify_external_signer_via_syscall;

use anchor_lang::solana_program::hash::Hasher;

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
    fn signers_mut(&mut self) -> &mut SmartAccountSignerWrapper;
    fn threshold(&self) -> u16;
    fn time_lock(&self) -> u32;
    fn transaction_index(&self) -> u64;
    fn set_transaction_index(&mut self, transaction_index: u64) -> Result<()>;
    fn stale_transaction_index(&self) -> u64;

    fn signers_v2(&self) -> Vec<SmartAccountSigner> {
        self.signers().as_v2()
    }

    fn is_signer_v2(&self, key: Pubkey) -> Option<SmartAccountSigner> {
        self.signers().find(&key)
    }

    fn find_signer_by_session_key(&self, pubkey: Pubkey, current_timestamp: u64) -> Option<SmartAccountSigner> {
        self.signers().find_by_session_key(&pubkey, current_timestamp)
    }

    /// Resolve the signer key for a given account key.
    ///
    /// For native signers: returns their key directly.
    /// For session keys: returns the parent external signer's key.
    /// For external signers: returns their key_id.
    fn resolve_signer_key(&self, signer_key: Pubkey, is_native_tx_signer: bool) -> Result<Pubkey> {
        // Fast path: if not a native tx signer, must be external
        if !is_native_tx_signer {
            for signer in self.signers_v2().into_iter() {
                if signer.is_external() && signer.key() == signer_key {
                    return Ok(signer.key());
                }
            }
            return Err(SmartAccountError::NotASigner.into());
        }

        let now = Clock::get()?.unix_timestamp as u64;

        // Slow path: native tx signer - check native match and session key match
        for signer in self.signers_v2().into_iter() {
            match signer.signer_type() {
                SignerType::Native => {
                    if signer.key() == signer_key {
                        return Ok(signer.key());
                    }
                }
                _ => {
                    // Check for session key match — return parent signer's key
                    // only if the session key has not expired.
                    if let Some(skd) = signer.get_session_key_data_if_matches(&signer_key) {
                        if skd.expiration > now {
                            return Ok(signer.key());
                        }
                    }
                }
            }
        }

        Err(SmartAccountError::NotASigner.into())
    }

    // Returns `Some(index)` if `signer_pubkey` is a signer, with `index` into the `signers` vec.
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

    /// Verify a signer's authenticity and return the resolved key.
    ///
    /// Determines signer type by scanning the signers list (native/session key/external),
    /// performs all necessary checks (permissions, session key expiration, external signature
    /// verification), and returns the canonical key (parent key for session keys).
    fn verify_signer(
        &mut self,
        signer_info: &AccountInfo,
        remaining_accounts: &[AccountInfo],
        non_hashed_message: Hasher,
        extra_verification_data: Option<&ExtraVerificationData>,
        required_permission: Option<Permission>,
    ) -> Result<Pubkey>
    where
        Self: Sized,
    {
        let signer_key = *signer_info.key;
        let is_native_tx_signer = signer_info.is_signer;
        let now = Clock::get()?.unix_timestamp as u64;

        // Fast path: if not a native tx signer, must be external
        if !is_native_tx_signer {
            for signer in self.signers_v2().into_iter() {
                if signer.is_external() && signer.key() == signer_key {
                    // External signer found — check permissions
                    if let Some(permission) = required_permission {
                        require!(
                            signer.permissions().has(permission),
                            SmartAccountError::Unauthorized
                        );
                    }

                    let evd = extra_verification_data
                        .ok_or(SmartAccountError::MissingExtraVerificationData)?;

                    let (sysvar_opt, _) = split_instructions_sysvar(remaining_accounts);
                    let (counter_update, next_nonce) = if evd.is_precompile() {
                        let sysvar = sysvar_opt
                            .ok_or(SmartAccountError::MissingPrecompileInstruction)?;
                        let results = verify_precompile_signers(
                            sysvar,
                            &[signer.clone()],
                            &[evd.clone()],
                            &non_hashed_message,
                        )?;
                        results.into_iter().next()
                            .ok_or_else(|| error!(SmartAccountError::MissingPrecompileInstruction))?
                    } else {
                        verify_external_signer_via_syscall(
                            &signer,
                            &non_hashed_message,
                            evd,
                        )?
                    };

                    let signer_key = signer.key();
                    if let Some(new_counter) = counter_update {
                        let updates = [(signer_key, new_counter)];
                        self.apply_counter_updates(&updates)?;
                    }
                    self.apply_nonce_update(&signer_key, next_nonce)?;

                    return Ok(signer_key);
                }
            }
            return Err(SmartAccountError::NotASigner.into());
        }

        // Slow path: native tx signer — check native match and session key match
        for signer in self.signers_v2().into_iter() {
            match signer.signer_type() {
                SignerType::Native => {
                    if signer.key() == signer_key {
                        // Native signer — check permissions and return
                        if let Some(permission) = required_permission {
                            require!(
                                signer.permissions().has(permission),
                                SmartAccountError::Unauthorized
                            );
                        }
                        return Ok(signer.key());
                    }
                }
                _ => {
                    // Check for session key match
                    if let Some(session_key_data) = signer.get_session_key_data_if_matches(&signer_key) {
                        // Session key — check expiration, then parent permissions
                        require!(
                            session_key_data.expiration > now,
                            SmartAccountError::InvalidSessionKeyExpiration
                        );
                        if let Some(permission) = required_permission {
                            require!(
                                signer.permissions().has(permission),
                                SmartAccountError::Unauthorized
                            );
                        }
                        return Ok(signer.key());
                    }
                }
            }
        }

        Err(SmartAccountError::NotASigner.into())
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
    /// Invariant: num_voters >= threshold (validated by Settings invariant).
    fn cutoff(&self) -> usize {
        self.num_voters()
            .saturating_sub(usize::from(self.threshold()))
            .saturating_add(1)
    }

    // Stale transaction protection (ported from Settings)
    fn invalidate_prior_transactions(&mut self);

    // Consensus validation (ported from Settings invariant)
    fn invariant(&self) -> Result<()>;

    fn apply_counter_updates(&mut self, _updates: &[(Pubkey, u64)]) -> Result<()> {
        Err(SmartAccountError::NotImplemented.into())
    }

    fn apply_nonce_update(&mut self, _key_id: &Pubkey, _nonce: u64) -> Result<()> {
        Err(SmartAccountError::NotImplemented.into())
    }
}
