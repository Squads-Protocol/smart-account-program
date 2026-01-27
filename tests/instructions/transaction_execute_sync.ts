describe("Instructions / transaction_execute_sync", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_execute_sync_transaction_v1");
  skip.it("should_execute_sync_transaction_v2");
  skip.it("should_execute_sync_policy_v2");
  skip.it("should_emit_sync_event_v1");
  skip.it("should_emit_sync_event_v2");
  skip.it("should_apply_counter_updates_for_v2_signers_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  skip.it("should_fail_when_consensus_account_inactive_v1");
  skip.it("should_fail_when_consensus_account_inactive_v2");
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
  skip.it("should_fail_on_invalid_account_index_locked_v1");
  skip.it("should_fail_on_invalid_account_index_locked_v2");
  skip.it("should_fail_on_invalid_payload_for_policy_v1");
  skip.it("should_fail_on_invalid_payload_for_policy_v2");
  skip.it("should_fail_on_invalid_payload_for_settings_v1");
  skip.it("should_fail_on_invalid_payload_for_settings_v2");
  skip.it("should_fail_on_invalid_v2_external_signer_verification_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");
  skip.it("should_fail_on_missing_policy_settings_account_when_required_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_execute_with_min_account_index_v1");
  skip.it("should_execute_with_min_account_index_v2");
  skip.it("should_execute_with_max_account_index_v1");
  skip.it("should_execute_with_max_account_index_v2");
  skip.it("should_execute_with_zero_timelock_v1");
  skip.it("should_execute_with_zero_timelock_v2");
  skip.it("should_execute_with_max_num_signers_v1");
  skip.it("should_execute_with_max_num_signers_v2");
  skip.it("should_execute_with_only_external_signers_v2");
  skip.it("should_execute_with_mixed_native_and_external_signers_v2");
  skip.it("should_execute_with_empty_instruction_list_v1");
  skip.it("should_execute_with_empty_instruction_list_v2");
  skip.it("should_execute_with_max_instruction_accounts_v1");
  skip.it("should_execute_with_max_instruction_accounts_v2");
  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
