#![allow(deprecated)]
use solana_program::pubkey::Pubkey;

/// Stores the data required for tracking the status of a smart account proposal.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone)]
pub struct Proposal {
    /// The consensus account (settings or policy) this belongs to.
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
    pub const DISCRIMINATOR: [u8; 8] = [0x1a, 0x5e, 0xbd, 0xbb, 0x74, 0x88, 0x35, 0x21];

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

    /// Check if the signer approved the transaction.
    pub fn has_voted_approve(&self, signer: Pubkey) -> Option<usize> {
        self.approved.binary_search(&signer).ok()
    }

    /// Check if the signer rejected the transaction.
    pub fn has_voted_reject(&self, signer: Pubkey) -> Option<usize> {
        self.rejected.binary_search(&signer).ok()
    }

    /// Delete the vote of rejection at the `index`.
    pub fn remove_rejection_vote(&mut self, index: usize) {
        self.rejected.remove(index);
    }

    /// Delete the vote of approval at the `index`.
    pub fn remove_approval_vote(&mut self, index: usize) {
        self.approved.remove(index);
    }
}

#[cfg(feature = "borsh")]
impl Proposal {
    pub fn try_deserialize(data: &[u8]) -> Result<Self, borsh::maybestd::io::Error> {
        if data.len() < 8 || data[..8] != Self::DISCRIMINATOR {
            return Err(borsh::maybestd::io::Error::new(
                borsh::maybestd::io::ErrorKind::InvalidData,
                "discriminator mismatch",
            ));
        }
        let mut body = &data[8..];
        <Self as borsh::BorshDeserialize>::deserialize(&mut body)
    }
}

/// The status of a proposal.
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Clone, PartialEq, Eq, Debug)]
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
    /// Proposal is being executed. Transient state.
    #[deprecated(note = "This status is no longer used.")]
    Executing,
    /// Proposal has been executed.
    Executed { timestamp: i64 },
    /// Proposal has been cancelled.
    Cancelled { timestamp: i64 },
}
