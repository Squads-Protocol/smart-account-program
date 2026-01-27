describe("Instructions / transaction_execute_sync_legacy", () => {
  const skip = { it: it.skip };

  // Golden Path Tests
  skip.it("should_execute_legacy_sync_transaction_v1");
  skip.it("should_emit_legacy_sync_event_v1");

  // Invariant & Safety Tests
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  skip.it("should_fail_on_non_settings_consensus_account_v1");
  skip.it("should_fail_on_invalid_account_index_locked_v1");
  skip.it("should_fail_on_threshold_not_met_v1");
  skip.it("should_fail_on_missing_permissions_v1");
  skip.it("should_fail_on_invalid_instruction_payload_v1");

  // Edge Case Tests
  skip.it("should_execute_with_min_account_index_v1");
  skip.it("should_execute_with_max_account_index_v1");
  skip.it("should_execute_with_zero_instructions_v1");
  skip.it("should_execute_with_max_instruction_accounts_v1");
});
