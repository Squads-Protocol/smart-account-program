describe("Instructions / proposal_vote", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_approve_proposal_v1");
  skip.it("should_approve_proposal_v2");
  skip.it("should_reject_proposal_v1");
  skip.it("should_reject_proposal_v2");
  skip.it("should_cancel_proposal_v1");
  skip.it("should_cancel_proposal_v2");
  skip.it("should_emit_vote_events_v1");
  skip.it("should_emit_vote_events_v2");
  skip.it("should_realloc_on_cancel_when_needed_v1");
  skip.it("should_realloc_on_cancel_when_needed_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  skip.it("should_fail_when_consensus_account_inactive_v1");
  skip.it("should_fail_when_consensus_account_inactive_v2");
  skip.it("should_fail_on_not_a_signer_v1");
  skip.it("should_fail_on_not_a_signer_v2");
  skip.it("should_fail_on_missing_vote_permission_v1");
  skip.it("should_fail_on_missing_vote_permission_v2");
  skip.it("should_fail_on_invalid_proposal_status_for_approve_v1");
  skip.it("should_fail_on_invalid_proposal_status_for_approve_v2");
  skip.it("should_fail_on_invalid_proposal_status_for_reject_v1");
  skip.it("should_fail_on_invalid_proposal_status_for_reject_v2");
  skip.it("should_fail_on_invalid_proposal_status_for_cancel_v1");
  skip.it("should_fail_on_invalid_proposal_status_for_cancel_v2");
  skip.it("should_fail_on_stale_proposal_for_approve_v1");
  skip.it("should_fail_on_stale_proposal_for_approve_v2");
  skip.it("should_fail_on_stale_proposal_for_reject_v1");
  skip.it("should_fail_on_stale_proposal_for_reject_v2");
  skip.it("should_fail_on_double_approve_v1");
  skip.it("should_fail_on_double_approve_v2");
  skip.it("should_fail_on_double_reject_v1");
  skip.it("should_fail_on_double_reject_v2");
  skip.it("should_fail_on_double_cancel_v1");
  skip.it("should_fail_on_double_cancel_v2");
  skip.it("should_fail_on_missing_payer_for_cancel_v1");
  skip.it("should_fail_on_missing_payer_for_cancel_v2");
  skip.it("should_fail_on_missing_system_program_for_cancel_v1");
  skip.it("should_fail_on_missing_system_program_for_cancel_v2");
  skip.it("should_fail_on_invalid_v2_context_signature_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_approve_at_threshold_v1");
  skip.it("should_approve_at_threshold_v2");
  skip.it("should_reject_at_cutoff_v1");
  skip.it("should_reject_at_cutoff_v2");
  skip.it("should_cancel_at_threshold_v1");
  skip.it("should_cancel_at_threshold_v2");
  skip.it("should_trim_cancelled_signers_not_in_settings_v1");
  skip.it("should_trim_cancelled_signers_not_in_settings_v2");
  skip.it("should_allow_empty_memo_v1");
  skip.it("should_allow_empty_memo_v2");
  skip.it("should_reject_oversized_memo_v1");
  skip.it("should_reject_oversized_memo_v2");
});
