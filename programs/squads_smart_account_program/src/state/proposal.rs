//! Proposal — re-exported from the types crate plus a `ProposalExt` extension
//! trait for methods that depend on `Clock::get`, AccountInfo, and other
//! anchor-side utilities.

#![allow(deprecated)]

use anchor_lang::prelude::*;

pub use squads_smart_account_program_types::{Proposal, ProposalStatus};

use crate::consensus_trait::ConsensusAccountType;
use crate::errors::*;
use crate::id;
use crate::utils::{self, realloc};
use crate::LogAuthorityInfo;
use crate::ProposalEvent;
use crate::ProposalEventType;
use crate::SmartAccountEvent;
use crate::SmartAccountEventExt;

/// Program-side extensions for `Proposal`: voting and account-lifecycle
/// methods that need Clock/AccountInfo.
pub trait ProposalExt {
    fn approve(&mut self, signer: Pubkey, threshold: usize) -> Result<()>;
    fn reject(&mut self, signer: Pubkey, cutoff: usize) -> Result<()>;
    fn cancel(&mut self, signer: Pubkey, threshold: usize) -> Result<()>;
    fn realloc_if_needed<'a>(
        proposal: AccountInfo<'a>,
        signers_length: usize,
        rent_payer: Option<AccountInfo<'a>>,
        system_program: Option<AccountInfo<'a>>,
    ) -> Result<bool>;
    fn close_if_exists<'info>(
        proposal_account: Option<Proposal>,
        proposal_info: AccountInfo<'info>,
        proposal_rent_collector: AccountInfo<'info>,
        log_authority_info: &LogAuthorityInfo<'info>,
        consensus_account_type: ConsensusAccountType,
    ) -> Result<()>;
}

impl ProposalExt for Proposal {
    /// Register an approval vote.
    fn approve(&mut self, signer: Pubkey, threshold: usize) -> Result<()> {
        // If `signer` has previously voted to reject, remove that vote.
        if let Some(vote_index) = self.has_voted_reject(signer.key()) {
            self.remove_rejection_vote(vote_index);
        }

        // Insert the vote of approval.
        match self.approved.binary_search(&signer) {
            Ok(_) => return err!(SmartAccountError::AlreadyApproved),
            Err(pos) => self.approved.insert(pos, signer),
        };

        // If current number of approvals reaches threshold, mark the transaction as `Approved`.
        if self.approved.len() >= threshold {
            self.status = ProposalStatus::Approved {
                timestamp: Clock::get()?.unix_timestamp,
            };
        }

        Ok(())
    }

    /// Register a rejection vote.
    fn reject(&mut self, signer: Pubkey, cutoff: usize) -> Result<()> {
        // If `signer` has previously voted to approve, remove that vote.
        if let Some(vote_index) = self.has_voted_approve(signer.key()) {
            self.remove_approval_vote(vote_index);
        }

        // Insert the vote of rejection.
        match self.rejected.binary_search(&signer) {
            Ok(_) => return err!(SmartAccountError::AlreadyRejected),
            Err(pos) => self.rejected.insert(pos, signer),
        };

        // If current number of rejections reaches cutoff, mark the transaction as `Rejected`.
        if self.rejected.len() >= cutoff {
            self.status = ProposalStatus::Rejected {
                timestamp: Clock::get()?.unix_timestamp,
            };
        }

        Ok(())
    }

    /// Register a cancellation vote.
    fn cancel(&mut self, signer: Pubkey, threshold: usize) -> Result<()> {
        // Insert the vote of cancellation.
        match self.cancelled.binary_search(&signer) {
            Ok(_) => return err!(SmartAccountError::AlreadyCancelled),
            Err(pos) => self.cancelled.insert(pos, signer),
        };

        // If current number of cancellations reaches threshold, mark the transaction as `Cancelled`.
        if self.cancelled.len() >= threshold {
            self.status = ProposalStatus::Cancelled {
                timestamp: Clock::get()?.unix_timestamp,
            };
        }

        Ok(())
    }

    /// Reallocate the proposal account if needed to accommodate `signers_length`.
    fn realloc_if_needed<'a>(
        proposal: AccountInfo<'a>,
        signers_length: usize,
        rent_payer: Option<AccountInfo<'a>>,
        system_program: Option<AccountInfo<'a>>,
    ) -> Result<bool> {
        // Sanity checks
        require_keys_eq!(
            *proposal.owner,
            id(),
            SmartAccountError::IllegalAccountOwner
        );

        // Check if we need to reallocate space.
        let current_account_size = proposal.data.borrow().len();
        let account_size_to_fit_signers = Proposal::size(signers_length);

        if current_account_size >= account_size_to_fit_signers {
            return Ok(false);
        }

        // Reallocate more space.
        realloc(
            &proposal,
            account_size_to_fit_signers,
            rent_payer,
            system_program,
        )?;

        Ok(true)
    }

    /// Close the proposal account if it exists.
    fn close_if_exists<'info>(
        proposal_account: Option<Proposal>,
        proposal_info: AccountInfo<'info>,
        proposal_rent_collector: AccountInfo<'info>,
        log_authority_info: &LogAuthorityInfo<'info>,
        consensus_account_type: ConsensusAccountType,
    ) -> Result<()> {
        if let Some(proposal) = proposal_account {
            require!(
                proposal_rent_collector.key() == proposal.rent_collector,
                SmartAccountError::InvalidRentCollector
            );
            let proposal_key = proposal_info.key();
            utils::close(proposal_info, proposal_rent_collector)?;
            let event = ProposalEvent {
                event_type: ProposalEventType::Close,
                consensus_account: log_authority_info.authority.key(),
                consensus_account_type,
                proposal_pubkey: proposal_key,
                transaction_index: proposal.transaction_index,
                signer: None,
                memo: None,
                proposal: None,
            };
            SmartAccountEvent::ProposalEvent(event).log(log_authority_info)?;
        }
        Ok(())
    }
}
