#![allow(deprecated)]
use anchor_lang::prelude::*;

use crate::errors::*;
use crate::id;
use crate::utils;
use crate::utils::realloc;

use anchor_lang::system_program;

/// Stores the data required for tracking the status of a smart account proposal.
/// Each `Proposal` has a 1:1 association with a transaction account, e.g. a `Transaction` or a `SettingsTransaction`;
/// the latter can be executed only after the `Proposal` has been approved and its time lock is released.
#[account]
pub struct Proposal {
    /// The settings this belongs to.
    pub settings: Pubkey,
    /// Index of the smart account transaction this proposal is associated with.
    pub transaction_index: u64,
    /// The rent collector for the proposal account.
    pub rent_collector: Pubkey,
    /// The status of the transaction.
    pub status: ProposalStatus,
    /// PDA bump.
    pub bump: u8,
    /// Keys that have approved/signed.
    pub approved: Vec<Pubkey>,
    /// Keys that have rejected.
    pub rejected: Vec<Pubkey>,
    /// Keys that have cancelled (Approved only).
    pub cancelled: Vec<Pubkey>,
}

impl Proposal {
    pub fn size(signers_len: usize) -> usize {
        8 +   // anchor account discriminator
        32 +  // settings
        8 +   // index
        32 +  // rent_payer
        1 +   // status enum variant
        8 +   // status enum wrapped timestamp (i64)
        1 +   // bump
        (4 + (signers_len * 32)) + // approved vec
        (4 + (signers_len * 32)) + // rejected vec
        (4 + (signers_len * 32)) // cancelled vec
    }

    /// Register an approval vote.
    pub fn approve(&mut self, signer: Pubkey, threshold: usize) -> Result<()> {
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
    pub fn reject(&mut self, signer: Pubkey, cutoff: usize) -> Result<()> {
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

    /// Registers a cancellation vote.
    pub fn cancel(&mut self, signer: Pubkey, threshold: usize) -> Result<()> {
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

    /// Check if the signer approved the transaction.
    /// Returns `Some(index)` if `signer` has approved the transaction, with `index` into the `approved` vec.
    fn has_voted_approve(&self, signer: Pubkey) -> Option<usize> {
        self.approved.binary_search(&signer).ok()
    }

    /// Check if the signer rejected the transaction.
    /// Returns `Some(index)` if `signer` has rejected the transaction, with `index` into the `rejected` vec.
    fn has_voted_reject(&self, signer: Pubkey) -> Option<usize> {
        self.rejected.binary_search(&signer).ok()
    }

    /// Delete the vote of rejection at the `index`.
    fn remove_rejection_vote(&mut self, index: usize) {
        self.rejected.remove(index);
    }

    /// Delete the vote of approval at the `index`.
    fn remove_approval_vote(&mut self, index: usize) {
        self.approved.remove(index);
    }

    /// Check if the proposal account space needs to be reallocated to accommodate `cancelled` vec.
    /// Proposal size is crated at creation, and thus may not accomodate enough space for all signers to cancel if more are added or changed
    /// Returns `true` if the account was reallocated.
    pub fn realloc_if_needed<'a>(
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

        let current_account_size = proposal.data.borrow().len();
        let account_size_to_fit_signers = Proposal::size(signers_length);

        // Check if we need to reallocate space.
        if current_account_size >= account_size_to_fit_signers {
            return Ok(false);
        }
        // Reallocate more space.
        realloc(&proposal, account_size_to_fit_signers, rent_payer, system_program)?;

        Ok(true)
    }

    /// Close the proposal account if it exists, transferring rent to the rent collector
    pub fn close_if_exists<'info>(
        proposal_account: Option<Proposal>,
        proposal_info: AccountInfo<'info>,
        proposal_rent_collector: AccountInfo<'info>,
    ) -> Result<()> {
        if let Some(proposal) = proposal_account {
            require!(
                proposal_rent_collector.key() == proposal.rent_collector,
                SmartAccountError::InvalidRentCollector
            );
            utils::close(
                proposal_info,
                proposal_rent_collector,
            )?;
        }
        Ok(())
    }
}

/// The status of a proposal.
/// Each variant wraps a timestamp of when the status was set.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum ProposalStatus {
    /// Proposal is in the draft mode and can be voted on.
    Draft { timestamp: i64 },
    /// Proposal is live and ready for voting.
    Active { timestamp: i64 },
    /// Proposal has been rejected.
    Rejected { timestamp: i64 },
    /// Proposal has been approved and is pending execution.
    Approved { timestamp: i64 },
    /// Proposal is being executed. This is a transient state that always transitions to `Executed` in the span of a single transaction.
    #[deprecated(
        note = "This status used to be used to prevent reentrancy attacks. It is no longer needed."
    )]
    Executing,
    /// Proposal has been executed.
    Executed { timestamp: i64 },
    /// Proposal has been cancelled.
    Cancelled { timestamp: i64 },
}
