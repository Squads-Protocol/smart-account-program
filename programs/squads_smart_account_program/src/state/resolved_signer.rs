use std::cell::OnceCell;
use std::collections::BTreeSet;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::Hasher;

use crate::consensus_trait::Consensus;
use crate::errors::SmartAccountError;
use crate::state::signer_v2::ExtraVerificationData;
use crate::Permission;

/// Signer wrapper with lazy key resolution and verification.
///
/// Unlike `InterfaceAccount`, this has NO owner check and NO
/// `AccountNotInitialized` guard — external signers can have 0 lamports.
///
/// Flow:
/// 1. `verify()` → verifies signer authenticity, caches resolved key
/// 2. `resolved_key()` in handler → reads cached key, zero cost
pub struct ResolvedSigner<'info> {
    info: AccountInfo<'info>,
    resolved_key: OnceCell<Pubkey>,
}

impl<'info> ResolvedSigner<'info> {
    /// Verify the signer against a consensus account and cache the resolved key.
    ///
    /// Delegates to `consensus.verify_signer()` which handles native, session key,
    /// and external signers. The returned canonical key is cached for later use.
    pub fn verify(
        &self,
        consensus: &mut impl Consensus,
        remaining_accounts: &[AccountInfo],
        message: Hasher,
        evd: Option<&ExtraVerificationData>,
        permission: Option<Permission>,
    ) -> Result<()> {
        require!(
            self.resolved_key.get().is_none(),
            SmartAccountError::SignerAlreadyVerified
        );
        let key = consensus.verify_signer(
            &self.info,
            remaining_accounts,
            message,
            evd,
            permission,
        )?;
        self.resolved_key
            .set(key)
            .expect("resolved_key must be empty after is_none check");
        Ok(())
    }

    /// Returns the resolved key. Errors if verify() hasn't been called.
    pub fn resolved_key(&self) -> Result<Pubkey> {
        self.resolved_key
            .get()
            .copied()
            .ok_or_else(|| error!(SmartAccountError::NotASigner))
    }

    pub fn to_account_info(&self) -> AccountInfo<'info> {
        self.info.clone()
    }
}

// --- Anchor trait implementations ---

impl<'info, B> Accounts<'info, B> for ResolvedSigner<'info> {
    fn try_accounts(
        _program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        _ix_data: &[u8],
        _bumps: &mut B,
        _reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        if accounts.is_empty() {
            return Err(ErrorCode::AccountNotEnoughKeys.into());
        }
        let info = accounts[0].clone();
        *accounts = &accounts[1..];
        Ok(ResolvedSigner {
            info,
            resolved_key: OnceCell::new(),
        })
    }
}

impl<'info> ToAccountMetas for ResolvedSigner<'info> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let is_signer = is_signer.unwrap_or(self.info.is_signer);
        let meta = match self.info.is_writable {
            false => AccountMeta::new_readonly(*self.info.key, is_signer),
            true => AccountMeta::new(*self.info.key, is_signer),
        };
        vec![meta]
    }
}

impl<'info> ToAccountInfos<'info> for ResolvedSigner<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.info.clone()]
    }
}

impl<'info> AccountsExit<'info> for ResolvedSigner<'info> {}

impl<'info> Key for ResolvedSigner<'info> {
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}

impl<'info> AsRef<AccountInfo<'info>> for ResolvedSigner<'info> {
    fn as_ref(&self) -> &AccountInfo<'info> {
        &self.info
    }
}

// Anchor client/CPI modules are defined in instructions/mod.rs
// (must be siblings of instruction files for the derive macro to find them).

// Dummy struct for Anchor IDL parser — it scans for #[derive(Accounts)]
// and panics if a composite field type isn't found. The real ResolvedSigner
// above has a manual Accounts impl which the IDL parser ignores.
#[doc(hidden)]
mod _resolved_signer_idl {
    use anchor_lang::prelude::*;
    #[derive(Accounts)]
    pub struct ResolvedSigner<'info> {
        /// CHECK: Signer account (native, session key, or external)
        pub info: AccountInfo<'info>,
    }
}

/// Empty bumps type — ResolvedSigner has no PDA seeds, but Anchor's
/// derive macro for composite fields expects this type to exist.
#[derive(Debug, Default, Clone)]
pub struct ResolvedSignerBumps {}

impl<'a> anchor_lang::Bumps for ResolvedSigner<'a> {
    type Bumps = ResolvedSignerBumps;
}

impl ResolvedSignerBumps {
    pub fn get(&self, _name: &str) -> Option<u8> {
        None
    }
}
