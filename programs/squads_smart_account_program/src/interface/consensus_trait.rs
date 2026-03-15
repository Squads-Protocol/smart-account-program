use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{errors::SmartAccountError, Permission, SmartAccountSigner, SmartAccountSignerWrapper, SessionKeyData};
use crate::state::SignerType;
use crate::state::signer_v2::ExtraVerificationData;
use crate::state::signer_v2::precompile::{
    split_instructions_sysvar,
    verify_precompile_signers,
};
use crate::utils::context_validation::verify_external_signer_via_syscall;

use anchor_lang::solana_program::hash::Hasher;

/// Result of signer classification
pub enum ClassifiedSigner {
    /// Native Solana signer (verified via AccountInfo.is_signer)
    Native {
        signer: SmartAccountSigner,
    },
    /// Session key signer (native tx signer using a session key of an external signer)
    SessionKey {
        parent_signer: SmartAccountSigner,  // The external signer
        session_key_data: SessionKeyData,   // Full session key data (pubkey + expiration)
    },
    /// External signer (requires precompile verification)
    External {
        signer: SmartAccountSigner,
    },
}

impl ClassifiedSigner {
    /// Get the signer that holds the permissions
    pub fn signer(&self) -> &SmartAccountSigner {
        match self {
            Self::Native { signer } => signer,
            Self::SessionKey { parent_signer, .. } => parent_signer,
            Self::External { signer } => signer,
        }
    }
}

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

    fn signers_v2(&self) -> Vec<SmartAccountSigner> {
        self.signers().as_v2()
    }

    fn is_signer_v2(&self, key: Pubkey) -> Option<SmartAccountSigner> {
        self.signers().find(&key)
    }

    fn find_signer_by_session_key(&self, pubkey: Pubkey, current_timestamp: u64) -> Option<SmartAccountSigner> {
        self.signers().find_by_session_key(&pubkey, current_timestamp)
    }

    /// Resolve the canonical signer key for a given account key.
    ///
    /// For native signers: returns their key directly.
    /// For session keys: returns the parent external signer's key.
    /// For external signers: returns their key_id.
    ///
    /// This is a lightweight operation (no cryptographic verification) used by
    /// handlers that need the canonical key after verify_signer has already run.
    fn resolve_canonical_key(&self, signer_key: Pubkey, is_native_tx_signer: bool) -> Result<Pubkey> {
        let classified = self.classify_signer(signer_key, is_native_tx_signer)?;
        Ok(classified.signer().key())
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

    /// Classify a signer and return all necessary data for verification
    fn classify_signer(
        &self,
        signer_key: Pubkey,
        is_native_tx_signer: bool,
    ) -> Result<ClassifiedSigner> {
        // Fast path: if not a native tx signer, must be external
        if !is_native_tx_signer {
            for signer in self.signers_v2().into_iter() {
                if signer.is_external() && signer.key() == signer_key {
                    return Ok(ClassifiedSigner::External { signer });
                }
            }
            return Err(SmartAccountError::NotASigner.into());
        }

        // Slow path: native tx signer - check all three types
        for signer in self.signers_v2().into_iter() {
            match signer.signer_type() {
                SignerType::Native => {
                    if signer.key() == signer_key {
                        return Ok(ClassifiedSigner::Native { signer });
                    }
                }
                _ => {
                    // Check for session key match (just match key, expiration checked in verify_signer)
                    if let Some(session_key_data) = signer.get_session_key_data_if_matches(&signer_key) {
                        return Ok(ClassifiedSigner::SessionKey {
                            parent_signer: signer,
                            session_key_data,
                        });
                    }
                    // If not a session key, continue searching (don't match as External in slow path)
                }
            }
        }

        Err(SmartAccountError::NotASigner.into())
    }

    /// Verify a signer's authenticity (handles native, session key, and external signers).
    ///
    /// # Arguments
    /// * `signer_info` - The account info for the signer
    /// * `remaining_accounts` - Additional accounts (may include instructions sysvar for external signers)
    /// * `non_hashed_message` - The message hasher (nonce will be appended for external signers)
    /// * `extra_verification_data` - Optional extra data for verification, interpreted based on signer type:
    ///   - P256Webauthn: Parsed as `ClientDataJsonReconstructionParams`
    ///   - Secp256k1/Ed25519External: Currently unused, reserved for future use
    /// * `required_permission` - Optional permission to check
    /// Verify a signer and return the **canonical key** (parent signer key for session keys).
    ///
    /// Callers must use the returned key for permission checks, vote recording, and
    /// creator tracking instead of `signer_info.key()`, which may be a session key pubkey.
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
        let now = Clock::get()?.unix_timestamp as u64;
        let signer_key = *signer_info.key;

        // STEP 1: Classify signer
        let classified = self.classify_signer(signer_key, signer_info.is_signer)?;

        // STEP 2: Match and verify
        match classified {
            ClassifiedSigner::Native { signer } => {
                // Verify native signer
                require!(signer_info.is_signer, SmartAccountError::MissingSignature);

                let canonical_key = signer.key();

                // Check permission if required
                if let Some(permission) = required_permission {
                    require!(
                        signer.permissions().has(permission),
                        SmartAccountError::Unauthorized
                    );
                }

                Ok(canonical_key)
            }

            ClassifiedSigner::SessionKey { parent_signer, session_key_data } => {
                // Verify session key is a native tx signer
                require!(signer_info.is_signer, SmartAccountError::MissingSignature);

                // Check session key hasn't expired
                require!(
                    session_key_data.expiration > now,
                    SmartAccountError::InvalidSessionKeyExpiration
                );

                // Return parent signer's canonical key (not session key pubkey)
                let canonical_key = parent_signer.key();

                // Check permission on parent signer
                if let Some(permission) = required_permission {
                    require!(
                        parent_signer.permissions().has(permission),
                        SmartAccountError::Unauthorized
                    );
                }

                Ok(canonical_key)
            }

            ClassifiedSigner::External { signer } => {
                let canonical_key = signer.key();

                // Check permission first
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
                    // Precompile: batch function with 1-element slice
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
                    // Syscall: direct per-signer verification
                    verify_external_signer_via_syscall(
                        &signer,
                        &non_hashed_message,
                        evd,
                    )?
                };

                // Apply counter update if needed
                if let Some(new_counter) = counter_update {
                    let updates = [(signer_key, new_counter)];
                    self.apply_counter_updates(&updates)?;
                }

                // Apply nonce update
                self.apply_nonce_update(&signer_key, next_nonce)?;

                Ok(canonical_key)
            }
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
