describe("Instructions / transaction_create_from_buffer", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_create_transaction_from_buffer_v1");
  skip.it("should_create_transaction_from_buffer_v2");
  skip.it("should_close_buffer_and_return_rent_v1");
  skip.it("should_close_buffer_and_return_rent_v2");
  skip.it("should_emit_create_event_v1");
  skip.it("should_emit_create_event_v2");
  skip.it("should_realloc_transaction_account_to_final_size_v1");
  skip.it("should_realloc_transaction_account_to_final_size_v2");
  skip.it("should_top_up_rent_if_needed_v1");
  skip.it("should_top_up_rent_if_needed_v2");
  skip.it("should_set_payload_type_transaction_v1");
  skip.it("should_set_payload_type_transaction_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  skip.it("should_fail_when_consensus_account_inactive_v1");
  skip.it("should_fail_when_consensus_account_inactive_v2");
  skip.it("should_fail_on_creator_not_matching_buffer_creator_v1");
  skip.it("should_fail_on_creator_not_matching_buffer_creator_v2");
  skip.it("should_fail_on_invalid_buffer_hash_v1");
  skip.it("should_fail_on_invalid_buffer_hash_v2");
  skip.it("should_fail_on_invalid_buffer_size_v1");
  skip.it("should_fail_on_invalid_buffer_size_v2");
  skip.it("should_fail_when_args_transaction_message_not_empty_v1");
  skip.it("should_fail_when_args_transaction_message_not_empty_v2");
  skip.it("should_fail_on_policy_payload_args_v1");
  skip.it("should_fail_on_policy_payload_args_v2");
  skip.it("should_fail_on_missing_initiate_permission_v1");
  skip.it("should_fail_on_missing_initiate_permission_v2");
  skip.it("should_fail_on_not_a_signer_v1");
  skip.it("should_fail_on_not_a_signer_v2");
  skip.it("should_fail_on_invalid_v2_context_signature_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");
  skip.it("should_fail_on_invalid_payload_for_settings_v1");
  skip.it("should_fail_on_invalid_payload_for_settings_v2");
  skip.it("should_fail_on_invalid_account_index_locked_v1");
  skip.it("should_fail_on_invalid_account_index_locked_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_create_with_min_account_index_v1");
  skip.it("should_create_with_min_account_index_v2");
  skip.it("should_create_with_max_account_index_v1");
  skip.it("should_create_with_max_account_index_v2");
  skip.it("should_handle_zero_ephemeral_signers_v1");
  skip.it("should_handle_zero_ephemeral_signers_v2");
  skip.it("should_create_with_max_ephemeral_signers_v1");
  skip.it("should_create_with_max_ephemeral_signers_v2");
  skip.it("should_fail_with_ephemeral_signers_overflow_v1");
  skip.it("should_fail_with_ephemeral_signers_overflow_v2");
  skip.it("should_fail_with_insufficient_rent_payer_balance_v1");
  skip.it("should_fail_with_insufficient_rent_payer_balance_v2");
  skip.it("should_reject_empty_buffer_v1");
  skip.it("should_reject_empty_buffer_v2");
  skip.it("should_reject_oversized_buffer_v1");
  skip.it("should_reject_oversized_buffer_v2");
  skip.it("should_fail_on_transaction_index_overflow_v1");
  skip.it("should_fail_on_transaction_index_overflow_v2");
  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
