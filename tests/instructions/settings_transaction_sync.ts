describe("Instructions / settings_transaction_sync", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_execute_sync_settings_transaction_v1");
  skip.it("should_execute_sync_settings_transaction_v2");
  skip.it("should_emit_sync_settings_event_v1");
  skip.it("should_emit_sync_settings_event_v2");
  skip.it("should_realloc_settings_when_needed_v1");
  skip.it("should_realloc_settings_when_needed_v2");
  skip.it("should_apply_counter_updates_for_v2_signers_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  skip.it("should_fail_on_non_settings_consensus_account_v1");
  skip.it("should_fail_on_non_settings_consensus_account_v2");
  skip.it("should_fail_on_controlled_settings_v1");
  skip.it("should_fail_on_controlled_settings_v2");
  skip.it("should_fail_on_invalid_actions_v1");
  skip.it("should_fail_on_invalid_actions_v2");
  skip.it("should_fail_on_missing_initiate_permission_v1");
  skip.it("should_fail_on_missing_initiate_permission_v2");
  skip.it("should_fail_on_missing_vote_permission_v1");
  skip.it("should_fail_on_missing_vote_permission_v2");
  skip.it("should_fail_on_missing_execute_permission_v1");
  skip.it("should_fail_on_missing_execute_permission_v2");
  skip.it("should_fail_on_threshold_not_met_v1");
  skip.it("should_fail_on_threshold_not_met_v2");
  skip.it("should_fail_on_duplicate_signers_v1");
  skip.it("should_fail_on_duplicate_signers_v2");
  skip.it("should_fail_on_invalid_v2_external_signer_verification_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");
  skip.it("should_fail_when_reallocation_missing_rent_payer_v1");
  skip.it("should_fail_when_reallocation_missing_rent_payer_v2");
  skip.it("should_fail_when_reallocation_missing_system_program_v1");
  skip.it("should_fail_when_reallocation_missing_system_program_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_execute_with_empty_actions_list_v1");
  skip.it("should_execute_with_empty_actions_list_v2");
  skip.it("should_execute_with_max_actions_list_v1");
  skip.it("should_execute_with_max_actions_list_v2");
  skip.it("should_execute_with_only_external_signers_v2");
  skip.it("should_execute_with_mixed_native_and_external_signers_v2");
  skip.it("should_execute_with_optional_rent_payer_different_from_signer_v1");
  skip.it("should_execute_with_optional_rent_payer_different_from_signer_v2");
});
