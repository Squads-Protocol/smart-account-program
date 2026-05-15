use solana_address::Address;

use crate::generated::accounts::Proposal;

pub trait ProposalExt {
    /// Anchor account size for a `Proposal` whose vote vecs may grow to
    /// `signers_len` entries each.
    fn size(signers_len: usize) -> usize;
    fn has_voted_approve(&self, signer: Address) -> Option<usize>;
    fn has_voted_reject(&self, signer: Address) -> Option<usize>;
    fn remove_approval_vote(&mut self, index: usize);
    fn remove_rejection_vote(&mut self, index: usize);
}

impl ProposalExt for Proposal {
    fn size(signers_len: usize) -> usize {
        8   // discriminator
        + 32  // settings
        + 8   // transaction_index
        + 32  // rent_collector
        + 1   // ProposalStatus enum variant tag
        + 8   // wrapped timestamp (i64)
        + 1   // bump
        + (4 + signers_len * 32)  // approved
        + (4 + signers_len * 32)  // rejected
        + (4 + signers_len * 32) // cancelled
    }

    fn has_voted_approve(&self, signer: Address) -> Option<usize> {
        self.approved.binary_search(&signer).ok()
    }

    fn has_voted_reject(&self, signer: Address) -> Option<usize> {
        self.rejected.binary_search(&signer).ok()
    }

    fn remove_approval_vote(&mut self, index: usize) {
        self.approved.remove(index);
    }

    fn remove_rejection_vote(&mut self, index: usize) {
        self.rejected.remove(index);
    }
}
