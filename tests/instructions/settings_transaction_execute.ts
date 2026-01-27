describe("Instructions / settings_transaction_execute", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_execute_settings_transaction_v1");
  skip.it("should_execute_settings_transaction_v2");
  skip.it("should_emit_execute_events_v1");
  skip.it("should_emit_execute_events_v2");
  skip.it("should_mark_proposal_executed_v1");
  skip.it("should_mark_proposal_executed_v2");
  skip.it("should_apply_actions_and_update_settings_v1");
  skip.it("should_apply_actions_and_update_settings_v2");
  skip.it("should_realloc_settings_when_needed_v1");
  skip.it("should_realloc_settings_when_needed_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_not_a_signer_v1");
  skip.it("should_fail_on_not_a_signer_v2");
  skip.it("should_fail_on_missing_execute_permission_v1");
  skip.it("should_fail_on_missing_execute_permission_v2");
  skip.it("should_fail_on_invalid_proposal_status_v1");
  skip.it("should_fail_on_invalid_proposal_status_v2");
  skip.it("should_fail_on_timelock_not_released_v1");
  skip.it("should_fail_on_timelock_not_released_v2");
  skip.it("should_fail_on_stale_proposal_v1");
  skip.it("should_fail_on_stale_proposal_v2");
  skip.it("should_fail_on_expired_spending_limit_action_v1");
  skip.it("should_fail_on_expired_spending_limit_action_v2");
  skip.it("should_fail_on_invalid_v2_context_signature_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");
  skip.it("should_fail_when_reallocation_missing_rent_payer_v1");
  skip.it("should_fail_when_reallocation_missing_rent_payer_v2");
  skip.it("should_fail_when_reallocation_missing_system_program_v1");
  skip.it("should_fail_when_reallocation_missing_system_program_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_execute_with_zero_timelock_v1");
  skip.it("should_execute_with_zero_timelock_v2");
  skip.it("should_execute_with_max_timelock_v1");
  skip.it("should_execute_with_max_timelock_v2");
  skip.it("should_execute_with_no_actions_v1");
  skip.it("should_execute_with_no_actions_v2");
  skip.it("should_execute_with_max_actions_v1");
  skip.it("should_execute_with_max_actions_v2");
  skip.it("should_execute_with_optional_rent_payer_different_from_signer_v1");
  skip.it("should_execute_with_optional_rent_payer_different_from_signer_v2");
});
