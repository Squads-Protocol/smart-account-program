describe("Instructions / batch_add_transaction", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_add_transaction_to_batch_v1");
  skip.it("should_add_transaction_to_batch_v2");
  skip.it("should_increment_batch_size_v1");
  skip.it("should_increment_batch_size_v2");
  skip.it("should_derive_ephemeral_signer_bumps_v1");
  skip.it("should_derive_ephemeral_signer_bumps_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_not_a_signer_v1");
  skip.it("should_fail_on_not_a_signer_v2");
  skip.it("should_fail_on_missing_initiate_permission_v1");
  skip.it("should_fail_on_missing_initiate_permission_v2");
  skip.it("should_fail_when_signer_not_batch_creator_v1");
  skip.it("should_fail_when_signer_not_batch_creator_v2");
  skip.it("should_fail_on_invalid_proposal_status_v1");
  skip.it("should_fail_on_invalid_proposal_status_v2");
  skip.it("should_fail_on_invalid_transaction_message_format_v1");
  skip.it("should_fail_on_invalid_transaction_message_format_v2");
  skip.it("should_fail_on_invalid_v2_context_signature_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");
  skip.it("should_fail_on_batch_size_overflow_v1");
  skip.it("should_fail_on_batch_size_overflow_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_add_with_zero_ephemeral_signers_v1");
  skip.it("should_add_with_zero_ephemeral_signers_v2");
  skip.it("should_add_with_max_ephemeral_signers_v1");
  skip.it("should_add_with_max_ephemeral_signers_v2");
  skip.it("should_reject_empty_transaction_message_v1");
  skip.it("should_reject_empty_transaction_message_v2");
  skip.it("should_reject_oversized_transaction_message_v1");
  skip.it("should_reject_oversized_transaction_message_v2");
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
