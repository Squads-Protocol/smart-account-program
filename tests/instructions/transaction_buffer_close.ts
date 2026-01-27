describe("Instructions / transaction_buffer_close", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_close_transaction_buffer_v1");
  skip.it("should_close_transaction_buffer_v2");
  skip.it("should_return_rent_to_creator_v1");
  skip.it("should_return_rent_to_creator_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_invalid_consensus_account_derivation_v1");
  skip.it("should_fail_on_invalid_consensus_account_derivation_v2");
  skip.it("should_fail_on_creator_not_matching_buffer_creator_v1");
  skip.it("should_fail_on_creator_not_matching_buffer_creator_v2");
  skip.it("should_fail_on_not_a_signer_v1");
  skip.it("should_fail_on_not_a_signer_v2");
  skip.it("should_fail_on_missing_initiate_permission_v1");
  skip.it("should_fail_on_missing_initiate_permission_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_close_buffer_after_partial_extension_v1");
  skip.it("should_close_buffer_after_partial_extension_v2");
  skip.it("should_close_buffer_after_full_extension_v1");
  skip.it("should_close_buffer_after_full_extension_v2");
  // -------------------------------------------------------------------------------------
  // Settings V2 Tests
  // -------------------------------------------------------------------------------------

  skip.it("should_run_with_native_settings_v2");
  skip.it("should_run_with_p256_webauthn_settings_v2");
  skip.it("should_run_with_secp256k1_settings_v2");
  skip.it("should_run_with_ed25519_external_settings_v2");
});
