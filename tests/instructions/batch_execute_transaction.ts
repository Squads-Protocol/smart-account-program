describe("Instructions / batch_execute_transaction", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_execute_batch_transaction_v1");
  skip.it("should_execute_batch_transaction_v2");
  skip.it("should_increment_executed_transaction_index_v1");
  skip.it("should_increment_executed_transaction_index_v2");
  skip.it("should_mark_proposal_executed_on_last_transaction_v1");
  skip.it("should_mark_proposal_executed_on_last_transaction_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_not_a_signer_v1");
  skip.it("should_fail_on_not_a_signer_v2");
  skip.it("should_fail_on_missing_execute_permission_v1");
  skip.it("should_fail_on_missing_execute_permission_v2");
  skip.it("should_fail_on_invalid_proposal_status_v1");
  skip.it("should_fail_on_invalid_proposal_status_v2");
  skip.it("should_fail_on_timelock_not_released_v1");
  skip.it("should_fail_on_timelock_not_released_v2");
  skip.it("should_fail_on_invalid_remaining_accounts_order_v1");
  skip.it("should_fail_on_invalid_remaining_accounts_order_v2");
  skip.it("should_fail_on_invalid_address_lookup_table_accounts_v1");
  skip.it("should_fail_on_invalid_address_lookup_table_accounts_v2");
  skip.it("should_fail_on_invalid_message_account_infos_v1");
  skip.it("should_fail_on_invalid_message_account_infos_v2");
  skip.it("should_fail_when_protected_accounts_modified_v1");
  skip.it("should_fail_when_protected_accounts_modified_v2");
  skip.it("should_fail_on_invalid_v2_context_signature_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_execute_with_empty_address_lookup_tables_v1");
  skip.it("should_execute_with_empty_address_lookup_tables_v2");
  skip.it("should_execute_with_max_ephemeral_signers_v1");
  skip.it("should_execute_with_max_ephemeral_signers_v2");
  skip.it("should_fail_with_invalid_ephemeral_signer_bumps_v1");
  skip.it("should_fail_with_invalid_ephemeral_signer_bumps_v2");
  skip.it("should_execute_no_op_instructions_v1");
  skip.it("should_execute_no_op_instructions_v2");
  skip.it("should_fail_on_executed_transaction_index_overflow_v1");
  skip.it("should_fail_on_executed_transaction_index_overflow_v2");
  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
