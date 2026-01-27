describe("Instructions / transaction_close", () => {
  const skip = { it: it.skip };

  // Golden Path Tests
  skip.it("should_close_settings_transaction_when_terminal_v1");
  skip.it("should_close_settings_transaction_when_stale_v1");
  skip.it("should_close_transaction_when_terminal_v1");
  skip.it("should_close_transaction_when_stale_and_not_approved_v1");
  skip.it("should_close_batch_transaction_last_in_batch_v1");
  skip.it("should_close_batch_when_empty_v1");
  skip.it("should_close_empty_policy_transaction_v1");
  skip.it("should_emit_close_event_for_transaction_v1");

  // Invariant & Safety Tests
  skip.it("should_fail_close_settings_transaction_on_invalid_proposal_status_v1");
  skip.it("should_fail_close_transaction_on_invalid_proposal_status_v1");
  skip.it("should_fail_close_transaction_when_proposal_approved_and_stale_v1");
  skip.it("should_fail_close_batch_transaction_when_not_last_in_batch_v1");
  skip.it("should_fail_close_batch_transaction_on_invalid_proposal_status_v1");
  skip.it("should_fail_close_batch_when_batch_not_empty_v1");
  skip.it("should_fail_close_batch_on_invalid_proposal_status_v1");
  skip.it("should_fail_on_invalid_rent_collector_v1");
  skip.it("should_fail_close_empty_policy_transaction_on_invalid_empty_policy_account_v1");

  // Edge Case Tests
  skip.it("should_close_when_proposal_account_missing_v1");
  skip.it("should_close_when_proposal_data_empty_v1");
  skip.it("should_close_settings_transaction_with_stale_draft_v1");
  skip.it("should_close_settings_transaction_with_stale_active_v1");
  skip.it("should_close_batch_with_stale_proposal_not_approved_v1");
  skip.it("should_fail_close_batch_with_stale_approved_proposal_v1");
  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
