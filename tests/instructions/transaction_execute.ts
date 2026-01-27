describe("Instructions / transaction_execute", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_execute_transaction_v1");
  skip.it("should_execute_transaction_v2");
  skip.it("should_execute_policy_transaction_v2");
  skip.it("should_emit_execute_events_v1");
  skip.it("should_emit_execute_events_v2");
  skip.it("should_mark_proposal_executed_v1");
  skip.it("should_mark_proposal_executed_v2");
  skip.it("should_update_policy_during_execution_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  skip.it("should_fail_when_consensus_account_inactive_v1");
  skip.it("should_fail_when_consensus_account_inactive_v2");
  skip.it("should_fail_on_not_a_signer_v1");
  skip.it("should_fail_on_not_a_signer_v2");
  skip.it("should_fail_on_missing_execute_permission_v1");
  skip.it("should_fail_on_missing_execute_permission_v2");
  skip.it("should_fail_on_invalid_proposal_status_v1");
  skip.it("should_fail_on_invalid_proposal_status_v2");
  skip.it("should_fail_on_timelock_not_released_v1");
  skip.it("should_fail_on_timelock_not_released_v2");
  skip.it("should_fail_on_invalid_v2_context_signature_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");
  skip.it("should_fail_on_invalid_remaining_accounts_order_v1");
  skip.it("should_fail_on_invalid_remaining_accounts_order_v2");
  skip.it("should_fail_on_invalid_address_lookup_table_accounts_v1");
  skip.it("should_fail_on_invalid_address_lookup_table_accounts_v2");
  skip.it("should_fail_on_invalid_message_account_infos_v1");
  skip.it("should_fail_on_invalid_message_account_infos_v2");
  skip.it("should_fail_when_protected_accounts_modified_v1");
  skip.it("should_fail_when_protected_accounts_modified_v2");
  skip.it("should_fail_on_missing_policy_settings_account_when_required_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_execute_with_min_account_index_v1");
  skip.it("should_execute_with_min_account_index_v2");
  skip.it("should_execute_with_max_account_index_v1");
  skip.it("should_execute_with_max_account_index_v2");
  skip.it("should_execute_with_zero_timelock_v1");
  skip.it("should_execute_with_zero_timelock_v2");
  skip.it("should_execute_with_max_timelock_v1");
  skip.it("should_execute_with_max_timelock_v2");
  skip.it("should_execute_with_max_ephemeral_signers_v1");
  skip.it("should_execute_with_max_ephemeral_signers_v2");
  skip.it("should_fail_with_invalid_ephemeral_signer_bumps_v1");
  skip.it("should_fail_with_invalid_ephemeral_signer_bumps_v2");
  skip.it("should_execute_with_empty_address_lookup_tables_v1");
  skip.it("should_execute_with_empty_address_lookup_tables_v2");
  skip.it("should_execute_with_no_op_instructions_v1");
  skip.it("should_execute_with_no_op_instructions_v2");
});
