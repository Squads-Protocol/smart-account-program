describe("Instructions / batch_create", () => {
  const skip = { it: it.skip };

  // Golden Path Tests (V1 + V2 parity)
  skip.it("should_create_batch_v1");
  skip.it("should_create_batch_v2");
  skip.it("should_initialize_batch_fields_v1");
  skip.it("should_initialize_batch_fields_v2");
  skip.it("should_increment_settings_transaction_index_v1");
  skip.it("should_increment_settings_transaction_index_v2");

  // Invariant & Safety Tests (V1 + V2 parity)
  skip.it("should_fail_on_not_a_signer_v1");
  skip.it("should_fail_on_not_a_signer_v2");
  skip.it("should_fail_on_missing_initiate_permission_v1");
  skip.it("should_fail_on_missing_initiate_permission_v2");
  skip.it("should_fail_on_invalid_v2_context_signature_v2");
  skip.it("should_fail_on_invalid_webauthn_client_data_v2");
  skip.it("should_fail_on_transaction_index_overflow_v1");
  skip.it("should_fail_on_transaction_index_overflow_v2");

  // Edge Case Tests (V1 + V2 parity)
  skip.it("should_create_with_min_account_index_v1");
  skip.it("should_create_with_min_account_index_v2");
  skip.it("should_create_with_max_account_index_v1");
  skip.it("should_create_with_max_account_index_v2");
  skip.it("should_allow_empty_memo_v1");
  skip.it("should_allow_empty_memo_v2");
  skip.it("should_reject_oversized_memo_v1");
  skip.it("should_reject_oversized_memo_v2");
  skip.it("should_fail_with_insufficient_rent_payer_balance_v1");
  skip.it("should_fail_with_insufficient_rent_payer_balance_v2");
});
