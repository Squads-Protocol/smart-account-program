describe("Instructions / transaction_buffer_create", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_create_transaction_buffer_v1");
  skip.it("should_create_transaction_buffer_v2");
  skip.it("should_set_buffer_fields_v1");
  skip.it("should_set_buffer_fields_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  skip.it("should_fail_on_not_a_signer_v1");
  skip.it("should_fail_on_not_a_signer_v2");
  skip.it("should_fail_on_missing_initiate_permission_v1");
  skip.it("should_fail_on_missing_initiate_permission_v2");
  skip.it("should_fail_on_final_buffer_size_exceeded_v1");
  skip.it("should_fail_on_final_buffer_size_exceeded_v2");
  skip.it("should_fail_on_invalid_v2_context_signature_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_create_with_min_buffer_index_v1");
  skip.it("should_create_with_min_buffer_index_v2");
  skip.it("should_create_with_max_buffer_index_v1");
  skip.it("should_create_with_max_buffer_index_v2");
  skip.it("should_create_with_zero_initial_buffer_v1");
  skip.it("should_create_with_zero_initial_buffer_v2");
  skip.it("should_create_with_final_buffer_size_at_limit_v1");
  skip.it("should_create_with_final_buffer_size_at_limit_v2");
  skip.it("should_fail_with_insufficient_rent_payer_balance_v1");
  skip.it("should_fail_with_insufficient_rent_payer_balance_v2");

  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
