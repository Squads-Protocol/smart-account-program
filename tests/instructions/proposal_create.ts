describe("Instructions / proposal_create", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_create_proposal_active_v1");
  skip.it("should_create_proposal_active_v2");
  skip.it("should_create_proposal_draft_v1");
  skip.it("should_create_proposal_draft_v2");
  skip.it("should_emit_create_event_v1");
  skip.it("should_emit_create_event_v2");
  skip.it("should_initialize_vote_vectors_v1");
  skip.it("should_initialize_vote_vectors_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  skip.it("should_fail_when_consensus_account_inactive_v1");
  skip.it("should_fail_when_consensus_account_inactive_v2");
  skip.it("should_fail_on_invalid_transaction_index_gt_current_v1");
  skip.it("should_fail_on_invalid_transaction_index_gt_current_v2");
  skip.it("should_fail_on_stale_transaction_index_v1");
  skip.it("should_fail_on_stale_transaction_index_v2");
  skip.it("should_fail_on_not_a_signer_v1");
  skip.it("should_fail_on_not_a_signer_v2");
  skip.it("should_fail_on_missing_initiate_and_vote_permissions_v1");
  skip.it("should_fail_on_missing_initiate_and_vote_permissions_v2");
  skip.it("should_fail_on_invalid_v2_context_signature_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_create_with_min_transaction_index_v1");
  skip.it("should_create_with_min_transaction_index_v2");
  skip.it("should_create_with_max_transaction_index_v1");
  skip.it("should_create_with_max_transaction_index_v2");
  skip.it("should_reject_duplicate_proposal_pda_v1");
  skip.it("should_reject_duplicate_proposal_pda_v2");
  skip.it("should_allow_empty_memo_v1");
  skip.it("should_allow_empty_memo_v2");
  skip.it("should_reject_oversized_memo_v1");
  skip.it("should_reject_oversized_memo_v2");
});
