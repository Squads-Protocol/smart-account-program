describe("Instructions / activate_proposal", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_activate_draft_proposal_v1");
  skip.it("should_activate_draft_proposal_v2");
  skip.it("should_transition_status_to_active_v1");
  skip.it("should_transition_status_to_active_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_not_a_signer_v1");
  skip.it("should_fail_on_not_a_signer_v2");
  skip.it("should_fail_on_missing_initiate_permission_v1");
  skip.it("should_fail_on_missing_initiate_permission_v2");
  skip.it("should_fail_on_invalid_proposal_status_v1");
  skip.it("should_fail_on_invalid_proposal_status_v2");
  skip.it("should_fail_on_stale_proposal_v1");
  skip.it("should_fail_on_stale_proposal_v2");
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  skip.it("should_fail_on_invalid_v2_context_signature_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_activate_with_min_transaction_index_v1");
  skip.it("should_activate_with_min_transaction_index_v2");
  skip.it("should_activate_with_max_transaction_index_v1");
  skip.it("should_activate_with_max_transaction_index_v2");
});
